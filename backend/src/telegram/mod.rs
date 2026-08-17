mod dialogs;
mod initialize;

use crate::{config::Config, detection, market::MarketClient, notifications};
use anyhow::{Result, anyhow};
use grammers_client::update::Update;
use grammers_mtsender::UpdatesConfiguration;
use serde_json::json;
use sqlx::PgPool;
use tokio::sync::broadcast;

pub async fn run(config: Config, pool: PgPool, events: broadcast::Sender<String>) -> Result<()> {
    tracing::info!("Starting Telegram client");
    let initialization = initialize::connect(&config).await?;
    let channel_count =
        dialogs::sync_channels(&initialization.client, &pool, &config.photos_dir).await?;
    tracing::info!(dialogs = channel_count, "Telegram dialog sync completed");

    // Log the transition from initial synchronization to live ingestion so it
    // is obvious that the process is waiting for new Telegram messages.
    let updates = initialization
        .client
        .stream_updates(
            initialization.updates_receiver,
            UpdatesConfiguration { catch_up: false },
        )
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    tracing::info!("Telegram live update stream started");
    let market = MarketClient::new()?;
    let mut stream = updates;

    loop {
        let update = stream.next().await?;
        let Update::NewMessage(message) = update else {
            continue;
        };
        let Some(channel_id) = message.peer_id().bare_id() else {
            continue;
        };
        let text = message.text().trim();
        if text.is_empty() {
            continue;
        }

        let monitored: Option<(String,)> = sqlx::query_as(
            "SELECT name FROM channels WHERE telegram_id=$1 AND kind='channel' AND is_ignored=FALSE"
        ).bind(channel_id).fetch_optional(&pool).await?;
        if monitored.is_none() {
            continue;
        }

        let candidates = detection::detect_addresses(text);
        if candidates.is_empty() {
            continue;
        }
        // grammers returns jiff::Timestamp, while SQLx/PostgreSQL in this
        // project use chrono::DateTime<Utc>.
        let sent_at =
            chrono::DateTime::<chrono::Utc>::from_timestamp(message.date().as_second(), 0)
                .expect("Telegram timestamp must fit chrono's supported range");
        let telegram_message_id = i64::from(message.id());
        let message_row_id: i64 = sqlx::query_scalar(
            "INSERT INTO telegram_messages(telegram_message_id,channel_id,body,sent_at) VALUES($1,$2,$3,$4) ON CONFLICT(channel_id,telegram_message_id) DO UPDATE SET body=EXCLUDED.body RETURNING id"
        ).bind(telegram_message_id).bind(channel_id).bind(text).bind(sent_at).fetch_one(&pool).await?;

        let mut ingested_any = false;
        for candidate in candidates {
            // TODO:
            // HOTPATH BOTTLENECK: ingest_candidate() below makes synchronous HTTP calls
            // (market.resolve + historical_market_cap, up to 20s timeout each) while
            // holding up this loop. The next Telegram update is not read from the
            // stream until this completes, so a slow/rate-limited API call stalls
            // ingestion for every channel, not just this one.
            // TODO: tokio::spawn each ingest_candidate call (with a semaphore to cap
            // concurrent outbound requests) so message intake never blocks on market
            // data resolution.

            // One malformed or temporarily unavailable token must not stop the
            // entire Telegram update stream and hide every later shill.
            match ingest_candidate(
                &pool,
                &market,
                channel_id,
                message_row_id,
                sent_at,
                &candidate.address,
            )
            .await
            {
                Ok(()) => ingested_any = true,
                Err(error) => {
                    tracing::error!(%error, address = candidate.address, channel_id, "Failed to ingest token candidate")
                }
            }
        }
        if ingested_any {
            let _ = events.send(json!({"type":"new_shill"}).to_string());
        }
    }
}

async fn ingest_candidate(
    pool: &PgPool,
    market: &MarketClient,
    channel_id: i64,
    message_row_id: i64,
    sent_at: chrono::DateTime<chrono::Utc>,
    address: &str,
) -> Result<()> {
    let resolved = market.resolve(address).await;
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM tokens WHERE LOWER(contract_address)=LOWER($1) ORDER BY (chain_id IS NULL), id LIMIT 1"
    ).bind(address).fetch_optional(pool).await?;
    let token_id = if let Some((id,)) = existing {
        id
    } else {
        sqlx::query_scalar("INSERT INTO tokens(contract_address) VALUES($1) RETURNING id")
            .bind(address)
            .fetch_one(pool)
            .await?
    };

    if let Ok(snapshot) = &resolved {
        sqlx::query("UPDATE tokens SET chain_id=$2,pair_address=$3,symbol=$4,name=$5,image_url=$6,current_market_cap=$7,website_url=$8,twitter_url=$9,telegram_url=$10,market_status='tracking',last_market_at=NOW(),last_market_error=NULL,updated_at=NOW() WHERE id=$1")
            .bind(token_id).bind(&snapshot.chain_id).bind(&snapshot.pair_address).bind(&snapshot.symbol)
            .bind(&snapshot.name).bind(&snapshot.image_url).bind(snapshot.current_market_cap)
            .bind(&snapshot.website_url).bind(&snapshot.twitter_url).bind(&snapshot.telegram_url).execute(pool).await?;
    } else {
        let detail = resolved.as_ref().unwrap_err().to_string();
        sqlx::query("UPDATE tokens SET market_status='unavailable',last_market_error=$2,updated_at=NOW() WHERE id=$1")
            .bind(token_id).bind(&detail).execute(pool).await?;
        let category = if detail.contains("429") {
            "dexscreener_rate_limit"
        } else {
            "dexscreener_ingestion_error"
        };
        // A failed first lookup prevents Initial MC creation, so notify the
        // operator immediately instead of leaving only an unavailable badge.
        notifications::important(
            category,
            &format!(
                "ShillTrace\nNew shill market lookup failed\nContract: {address}\nError: {detail}"
            ),
        )
        .await;
    }

    let active_period: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM tracking_periods WHERE token_id=$1 AND status='active'")
            .bind(token_id)
            .fetch_optional(pool)
            .await?;
    let period_id = match active_period {
        Some((id,)) => id,
        None => sqlx::query_scalar("INSERT INTO tracking_periods(token_id,started_at,highest_market_cap) VALUES($1,$2,$3) RETURNING id")
            .bind(token_id).bind(sent_at).bind(resolved.as_ref().ok().map(|v| v.current_market_cap)).fetch_one(pool).await?,
    };

    let existing_shill: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM shills WHERE tracking_period_id=$1 AND channel_id=$2")
            .bind(period_id)
            .bind(channel_id)
            .fetch_optional(pool)
            .await?;
    if let Some((shill_id,)) = existing_shill {
        sqlx::query("INSERT INTO shill_messages(shill_id,telegram_message_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(shill_id).bind(message_row_id).execute(pool).await?;
        return Ok(());
    }

    let initial = match &resolved {
        Ok(snapshot) => match market
            .historical_market_cap(snapshot, address, sent_at)
            .await
        {
            Ok(value) => Some(value),
            Err(error)
                if chrono::Utc::now()
                    .signed_duration_since(sent_at)
                    .num_minutes()
                    <= 5 =>
            {
                // Some new chains (currently Robinhood) are present on DEX
                // Screener before GeckoTerminal supports historical candles.
                // For live Telegram shills, the immediately resolved market cap
                // is the closest available value to the message timestamp.
                tracing::warn!(%error, chain = snapshot.chain_id, address, "Historical candle unavailable; using live shill market cap");
                Some(snapshot.current_market_cap)
            }
            Err(error) => {
                tracing::warn!(%error, address, "Initial market cap unavailable");
                None
            }
        },
        Err(_) => None,
    };
    let shill_id: i64 = sqlx::query_scalar("INSERT INTO shills(tracking_period_id,token_id,channel_id,first_message_id,shilled_at,initial_market_cap,max_market_cap,market_status) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id")
        .bind(period_id).bind(token_id).bind(channel_id).bind(message_row_id).bind(sent_at)
        .bind(initial).bind(resolved.as_ref().ok().map(|v| v.current_market_cap))
        .bind(if initial.is_some() { "tracking" } else { "unavailable" }).fetch_one(pool).await?;
    sqlx::query("INSERT INTO shill_messages(shill_id,telegram_message_id) VALUES($1,$2)")
        .bind(shill_id)
        .bind(message_row_id)
        .execute(pool)
        .await?;
    Ok(())
}
