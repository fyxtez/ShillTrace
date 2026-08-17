use crate::{market::MarketClient, notifications};
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use tokio::{sync::broadcast, time::{Duration, interval}};

pub fn spawn(pool: PgPool, market: MarketClient, events: broadcast::Sender<String>, seconds: u64) {
    tokio::spawn(async move {
        let mut timer = interval(Duration::from_secs(seconds.max(5)));
        loop {
            timer.tick().await;
            if let Err(error) = poll_once(&pool, &market, &events).await {
                tracing::warn!(%error, "market tracking cycle failed");
                notifications::important(
                    "market_tracking_cycle",
                    &format!("ShillTrace\nMarket tracking cycle failed\nError: {error}"),
                )
                .await;
            }
        }
    });
}

async fn poll_once(pool: &PgPool, market: &MarketClient, events: &broadcast::Sender<String>) -> anyhow::Result<()> {
    let active: Vec<(i64, i64, String, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT t.id, p.id, t.contract_address, p.started_at FROM tokens t JOIN tracking_periods p ON p.token_id=t.id AND p.status='active' WHERE t.market_status='tracking'"
    ).fetch_all(pool).await?;

    for (token_id, period_id, address, started_at) in active {
        match market.resolve(&address).await {
            Ok(snapshot) => {
                let now = Utc::now();
                let mut tx = pool.begin().await?;
                // Refresh socials together with market data because projects
                // frequently add official links after the first live pair.
                sqlx::query("UPDATE tokens SET current_market_cap=$2,last_market_at=$3,website_url=$4,twitter_url=$5,telegram_url=$6,last_market_error=NULL,updated_at=NOW() WHERE id=$1")
                    .bind(token_id).bind(snapshot.current_market_cap).bind(now)
                    .bind(&snapshot.website_url).bind(&snapshot.twitter_url).bind(&snapshot.telegram_url).execute(&mut *tx).await?;
                sqlx::query("UPDATE tracking_periods SET highest_market_cap=GREATEST(COALESCE(highest_market_cap,0),$2) WHERE id=$1")
                    .bind(period_id).bind(snapshot.current_market_cap).execute(&mut *tx).await?;
                sqlx::query("UPDATE shills SET max_market_cap=GREATEST(COALESCE(max_market_cap,0),$2) WHERE tracking_period_id=$1")
                    .bind(period_id).bind(snapshot.current_market_cap).execute(&mut *tx).await?;
                // The first successful live sample is the best recoverable
                // Initial MC when a chain has no historical candle provider.
                // This guarantees every actively resolved shill receives an
                // initial baseline and therefore usable Current/Max X values.
                sqlx::query("UPDATE shills SET initial_market_cap=COALESCE(initial_market_cap,$2),max_market_cap=GREATEST(COALESCE(max_market_cap,0),$2),market_status='tracking' WHERE tracking_period_id=$1")
                    .bind(period_id).bind(snapshot.current_market_cap).execute(&mut *tx).await?;
                // One-minute samples are frequent enough to explain movement
                // without producing a noisy 15-second chart or excess rows.
                let age = now.signed_duration_since(started_at).num_seconds();
                let bucket = if age <= 604_800 { 60 } else if age <= 2_592_000 { 300 } else { 3_600 };
                let sampled_at = chrono::DateTime::from_timestamp((now.timestamp() / bucket) * bucket, 0).unwrap_or(now);
                sqlx::query("INSERT INTO market_cap_samples(token_id,tracking_period_id,market_cap,recorded_at) VALUES($1,$2,$3,$4) ON CONFLICT(token_id,tracking_period_id,recorded_at) DO UPDATE SET market_cap=EXCLUDED.market_cap")
                    .bind(token_id).bind(period_id).bind(snapshot.current_market_cap).bind(sampled_at).execute(&mut *tx).await?;
                tx.commit().await?;
                let _ = events.send(json!({"type":"market_update","token_id":token_id,"market_cap":snapshot.current_market_cap}).to_string());
            }
            Err(error) => {
                // Missing data remains informational; only the user can stop a
                // token, so a temporary DEX outage never silently removes it.
                sqlx::query("UPDATE tokens SET last_market_error=$2,updated_at=NOW() WHERE id=$1")
                    .bind(token_id).bind(error.to_string()).execute(pool).await?;
                let detail = error.to_string();
                let category = if detail.contains("429") { "dexscreener_rate_limit" } else { "dexscreener_market_error" };
                notifications::important(
                    category,
                    &format!("ShillTrace\nDEX Screener market lookup failed\nToken: {address}\nError: {detail}"),
                )
                .await;
            }
        }
    }
    Ok(())
}
