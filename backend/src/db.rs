use chrono::{DateTime, Utc};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::collections::BTreeMap;

pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    repair_market_cap_excursions(&pool).await?;
    Ok(pool)
}

fn ratio(a: f64, b: f64) -> f64 {
    if a <= 0.0 || b <= 0.0 {
        1.0
    } else {
        (a / b).max(b / a)
    }
}

async fn repair_market_cap_excursions(pool: &PgPool) -> anyhow::Result<()> {
    // This repair targets data written by the old broad-pair polling bug and is
    // intentionally one-shot; normal startups must not scan the full sample table.
    let already_done: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM maintenance_flags WHERE key='repair_market_cap_excursions_v1')",
    )
    .fetch_one(pool)
    .await?;
    if already_done {
        return Ok(());
    }
    let rows: Vec<(i64, DateTime<Utc>, f64)> = sqlx::query_as(
        "SELECT tracking_period_id,recorded_at,market_cap FROM market_cap_samples ORDER BY tracking_period_id,recorded_at",
    )
    .fetch_all(pool)
    .await?;
    let mut periods: BTreeMap<i64, Vec<(DateTime<Utc>, f64)>> = BTreeMap::new();
    for (period_id, recorded_at, market_cap) in rows {
        periods
            .entry(period_id)
            .or_default()
            .push((recorded_at, market_cap));
    }

    for (period_id, samples) in periods {
        if samples.len() < 3 {
            continue;
        }
        let mut segments: Vec<(usize, usize)> = Vec::new();
        let mut start = 0usize;
        for index in 1..samples.len() {
            let seconds = samples[index]
                .0
                .signed_duration_since(samples[index - 1].0)
                .num_seconds()
                .abs();
            // The old broad DEX search occasionally switched pools and created a
            // multi-minute billion-dollar island between normal samples. Only a
            // >=100x jump inside five minutes creates a boundary, preserving real
            // long-term pumps while identifying this specific provider corruption.
            if seconds <= 300 && ratio(samples[index - 1].1, samples[index].1) >= 100.0 {
                segments.push((start, index - 1));
                start = index;
            }
        }
        segments.push((start, samples.len() - 1));
        if segments.len() < 3 {
            continue;
        }

        let mut repaired = false;
        for segment_index in 1..segments.len() - 1 {
            let (middle_start, middle_end) = segments[segment_index];
            let (_, previous_end) = segments[segment_index - 1];
            let (next_start, _) = segments[segment_index + 1];
            let previous = samples[previous_end].1;
            let middle_first = samples[middle_start].1;
            let middle_last = samples[middle_end].1;
            let next = samples[next_start].1;
            if ratio(previous, middle_first) >= 100.0
                && ratio(middle_last, next) >= 100.0
                && ratio(previous, next) <= 5.0
            {
                sqlx::query("DELETE FROM market_cap_samples WHERE tracking_period_id=$1 AND recorded_at BETWEEN $2 AND $3")
                    .bind(period_id)
                    .bind(samples[middle_start].0)
                    .bind(samples[middle_end].0)
                    .execute(pool)
                    .await?;
                repaired = true;
                tracing::warn!(period_id, from = %samples[middle_start].0, to = %samples[middle_end].0, "Removed isolated corrupted market-cap excursion");
            }
        }

        if repaired {
            let corrected_max: Option<f64> = sqlx::query_scalar(
                r#"
                SELECT MAX(value) FROM (
                    SELECT market_cap AS value FROM market_cap_samples WHERE tracking_period_id=$1
                    UNION ALL
                    SELECT current_market_cap FROM tokens t JOIN tracking_periods p ON p.token_id=t.id WHERE p.id=$1 AND current_market_cap IS NOT NULL
                    UNION ALL
                    SELECT initial_market_cap FROM shills WHERE tracking_period_id=$1 AND initial_market_cap IS NOT NULL
                ) values_to_keep
                "#,
            )
            .bind(period_id)
            .fetch_one(pool)
            .await?;
            // Max X is derived from persisted maxima, so repairing chart samples
            // must repair both tracking and shill maxima in the same startup pass.
            if let Some(corrected_max) = corrected_max {
                sqlx::query("UPDATE tracking_periods SET highest_market_cap=$2 WHERE id=$1")
                    .bind(period_id)
                    .bind(corrected_max)
                    .execute(pool)
                    .await?;
                sqlx::query("UPDATE shills SET max_market_cap=$2 WHERE tracking_period_id=$1")
                    .bind(period_id)
                    .bind(corrected_max)
                    .execute(pool)
                    .await?;
            }
        }
    }
    sqlx::query("INSERT INTO maintenance_flags(key) VALUES('repair_market_cap_excursions_v1') ON CONFLICT DO NOTHING")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn reclassify_token_as_wallet(
    pool: &PgPool,
    token_id: i64,
    address: &str,
    chain_id: &str,
) -> anyhow::Result<i64> {
    let mut tx = pool.begin().await?;
    // Reclassification happens after the hot-path placeholder is committed. All
    // message references move atomically so sender/timestamp history survives,
    // while the address disappears from token tracking and channel performance.
    let wallet_id: i64 = sqlx::query_scalar(
        "INSERT INTO wallets(chain_id,address) VALUES($1,$2) ON CONFLICT (chain_id, LOWER(address)) DO UPDATE SET updated_at=NOW() RETURNING id",
    )
    .bind(chain_id)
    .bind(address)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO wallet_mentions(wallet_id,channel_id,telegram_message_id,mentioned_at)
        SELECT $2,s.channel_id,sm.telegram_message_id,m.sent_at
        FROM shills s
        JOIN shill_messages sm ON sm.shill_id=s.id
        JOIN telegram_messages m ON m.id=sm.telegram_message_id
        WHERE s.token_id=$1
        ON CONFLICT(wallet_id,telegram_message_id) DO NOTHING
        "#,
    )
    .bind(token_id)
    .bind(wallet_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM market_cap_samples WHERE token_id=$1")
        .bind(token_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM shills WHERE token_id=$1")
        .bind(token_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM tracking_periods WHERE token_id=$1")
        .bind(token_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM tokens WHERE id=$1")
        .bind(token_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(wallet_id)
}
