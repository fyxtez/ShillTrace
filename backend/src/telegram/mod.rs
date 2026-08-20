mod dialogs;
mod initialize;

use crate::{config::Config, db, detection, market::MarketClient, notifications};
use anyhow::{Result, anyhow};
use grammers_client::update::Update;
use grammers_mtsender::UpdatesConfiguration;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::{Semaphore, broadcast};

struct PendingEnrichment {
    token_id: i64,
    period_id: i64,
    shill_id: i64,
    address: String,
    wallet_chain_hint: Option<&'static str>,
    sent_at: chrono::DateTime<chrono::Utc>,
}

enum FastIngestOutcome {
    NewToken(PendingEnrichment),
    NewWallet,
    Duplicate,
}

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
    // Market enrichment is detached from Telegram intake, but a shared limit
    // prevents a burst of shills from overwhelming DEX providers or the DB.
    let enrichment_limit = Arc::new(Semaphore::new(4));
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

        let mut pending_enrichments = Vec::new();
        let mut new_wallet = false;
        for candidate in candidates {
            // Persist the minimum user-visible record before any external HTTP
            // call so frontend notification latency depends only on Telegram
            // delivery and PostgreSQL, not DEX/Gecko response time.
            let wallet_chain_hint = detection::wallet_chain_hint(text, candidate.address_kind);
            match ingest_candidate_fast(
                &pool,
                channel_id,
                message_row_id,
                sent_at,
                &candidate.address,
                wallet_chain_hint,
            )
            .await
            {
                Ok(FastIngestOutcome::NewToken(pending)) => pending_enrichments.push(pending),
                Ok(FastIngestOutcome::NewWallet) => new_wallet = true,
                // Repeated token/wallet mentions are stored but do not create a
                // duplicate alert for the same Telegram message.
                Ok(FastIngestOutcome::Duplicate) => {}
                Err(error) => {
                    tracing::error!(%error, address = candidate.address, channel_id, "Failed to ingest token candidate")
                }
            }
        }
        if !pending_enrichments.is_empty() {
            // Every minimal shill is committed before this event, so the first
            // frontend refresh can display it immediately with resolving data.
            let _ = events.send(json!({"type":"new_shill"}).to_string());
        }
        if new_wallet {
            // Known wallets stay on the DB-only hot path and get their own event,
            // so repeat wallet calls appear instantly without any market request.
            let _ = events.send(json!({"type":"new_wallet"}).to_string());
        }

        if !pending_enrichments.is_empty() {
            for pending in pending_enrichments {
                let task_pool = pool.clone();
                let task_market = market.clone();
                let task_events = events.clone();
                let task_limit = enrichment_limit.clone();
                tokio::spawn(async move {
                    let Ok(_permit) = task_limit.acquire_owned().await else {
                        return;
                    };
                    if let Err(error) = enrich_candidate(
                        &task_pool,
                        &task_market,
                        &task_events,
                        &pending,
                    ).await {
                        tracing::error!(%error, address = %pending.address, shill_id = pending.shill_id, "Background shill enrichment failed");
                    }
                });
            }
        }
    }
}

async fn ingest_candidate_fast(
    pool: &PgPool,
    channel_id: i64,
    message_row_id: i64,
    sent_at: chrono::DateTime<chrono::Utc>,
    address: &str,
    wallet_chain_hint: Option<&'static str>,
) -> Result<FastIngestOutcome> {
    // Once an address has been verified as a wallet, future mentions bypass token
    // creation and DEX enrichment entirely. This adds only a local indexed lookup
    // and keeps the network-free Telegram hot path intact.
    if let Some(chain_hint) = wallet_chain_hint {
        let known_wallet: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM wallets WHERE chain_id=$1 AND LOWER(address)=LOWER($2)",
        )
        .bind(chain_hint)
        .bind(address)
        .fetch_optional(pool)
        .await?;
        if let Some((wallet_id,)) = known_wallet {
            let inserted = sqlx::query(
                "INSERT INTO wallet_mentions(wallet_id,channel_id,telegram_message_id,mentioned_at) VALUES($1,$2,$3,$4) ON CONFLICT(wallet_id,telegram_message_id) DO NOTHING",
            )
            .bind(wallet_id)
            .bind(channel_id)
            .bind(message_row_id)
            .bind(sent_at)
            .execute(pool)
            .await?
            .rows_affected();
            return Ok(if inserted > 0 { FastIngestOutcome::NewWallet } else { FastIngestOutcome::Duplicate });
        }
    }

    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM tokens WHERE LOWER(contract_address)=LOWER($1) ORDER BY (chain_id IS NULL), id LIMIT 1"
    ).bind(address).fetch_optional(pool).await?;
    let token_id = if let Some((id,)) = existing {
        id
    } else {
        // `resolving` distinguishes a freshly visible shill from a genuine
        // provider failure while its background market lookup is in flight.
        sqlx::query_scalar("INSERT INTO tokens(contract_address,market_status) VALUES($1,'resolving') RETURNING id")
            .bind(address)
            .fetch_one(pool)
            .await?
    };

    let active_period: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM tracking_periods WHERE token_id=$1 AND status='active'")
            .bind(token_id)
            .fetch_optional(pool)
            .await?;
    let period_id = match active_period {
        Some((id,)) => id,
        None => sqlx::query_scalar("INSERT INTO tracking_periods(token_id,started_at,highest_market_cap) VALUES($1,$2,$3) RETURNING id")
            .bind(token_id).bind(sent_at).bind(Option::<f64>::None).fetch_one(pool).await?,
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
        return Ok(FastIngestOutcome::Duplicate);
    }

    // The placeholder is intentionally complete enough for list_shills: it
    // contains source/message identity now, while market fields arrive later.
    let shill_id: i64 = sqlx::query_scalar("INSERT INTO shills(tracking_period_id,token_id,channel_id,first_message_id,shilled_at,market_status) VALUES($1,$2,$3,$4,$5,'resolving') RETURNING id")
        .bind(period_id).bind(token_id).bind(channel_id).bind(message_row_id).bind(sent_at).fetch_one(pool).await?;
    sqlx::query("INSERT INTO shill_messages(shill_id,telegram_message_id) VALUES($1,$2)")
        .bind(shill_id)
        .bind(message_row_id)
        .execute(pool)
        .await?;
    Ok(FastIngestOutcome::NewToken(PendingEnrichment {
        token_id,
        period_id,
        shill_id,
        address: address.to_owned(),
        wallet_chain_hint,
        sent_at,
    }))
}

async fn enrich_candidate(
    pool: &PgPool,
    market: &MarketClient,
    events: &broadcast::Sender<String>,
    pending: &PendingEnrichment,
) -> Result<()> {
    let resolved = market.resolve(&pending.address).await;
    if let Ok(snapshot) = &resolved {
        sqlx::query("UPDATE tokens SET chain_id=$2,pair_address=$3,symbol=$4,name=$5,image_url=$6,current_market_cap=$7,website_url=$8,twitter_url=$9,telegram_url=$10,market_status='tracking',last_market_at=NOW(),last_market_error=NULL,updated_at=NOW() WHERE id=$1")
            .bind(pending.token_id).bind(&snapshot.chain_id).bind(&snapshot.pair_address).bind(&snapshot.symbol)
            .bind(&snapshot.name).bind(&snapshot.image_url).bind(snapshot.current_market_cap)
            .bind(&snapshot.website_url).bind(&snapshot.twitter_url).bind(&snapshot.telegram_url).execute(pool).await?;
    } else {
        let detail = resolved.as_ref().unwrap_err().to_string();
        // Only a genuine "no pair" result is eligible for wallet probing. Rate
        // limits and provider failures leave the fast placeholder untouched so a
        // temporary outage can never misclassify a token as a wallet.
        if detail.contains("DEX Screener found no pair with market-cap data") {
            let wallet_chain = if let Some(chain_hint) = pending.wallet_chain_hint {
                match market.is_wallet(&pending.address, chain_hint).await {
                    Ok(true) => Some(chain_hint),
                    Ok(false) => None,
                    Err(error) => {
                        tracing::warn!(%error, address = %pending.address, chain = chain_hint, "Wallet verification failed; keeping unresolved token");
                        None
                    }
                }
            } else if pending.address.starts_with("0x") {
                // Raw EVM addresses are chain-ambiguous. This discovery runs only
                // after DEX resolution fails and outside the Telegram hot path.
                market.discover_evm_wallet_chain(&pending.address).await
            } else {
                None
            };

            if let Some(chain_id) = wallet_chain {
                let wallet_id = db::reclassify_token_as_wallet(
                    pool, pending.token_id, &pending.address, chain_id,
                ).await?;
                let _ = events.send(json!({"type":"candidate_reclassified_wallet","wallet_id":wallet_id}).to_string());
                return Ok(());
            }
        }
        sqlx::query("UPDATE tokens SET market_status='unavailable',last_market_error=$2,updated_at=NOW() WHERE id=$1")
            .bind(pending.token_id).bind(&detail).execute(pool).await?;
        sqlx::query("UPDATE shills SET market_status='unavailable' WHERE id=$1")
            .bind(pending.shill_id).execute(pool).await?;
        let category = if detail.contains("429") {
            "dexscreener_rate_limit"
        } else {
            "dexscreener_ingestion_error"
        };
        // Failed enrichment is now reported from its background task; the
        // immediate frontend shill remains available for manual inspection.
        notifications::important(
            category,
            &format!(
                "ShillTrace\nNew shill market lookup failed\nContract: {}\nError: {detail}",
                pending.address
            ),
        )
        .await;
        let _ = events.send(json!({"type":"shill_updated","shill_id":pending.shill_id}).to_string());
        return Ok(());
    }

    let initial = match &resolved {
        Ok(snapshot) => match market
            .historical_market_cap(snapshot, &pending.address, pending.sent_at)
            .await
        {
            Ok(value) => Some(value),
            Err(error)
                if chrono::Utc::now()
                    .signed_duration_since(pending.sent_at)
                    .num_minutes()
                    <= 5 =>
            {
                // Some new chains (currently Robinhood) are present on DEX
                // Screener before GeckoTerminal supports historical candles.
                // For live Telegram shills, the immediately resolved market cap
                // is the closest available value to the message timestamp.
                tracing::warn!(%error, chain = snapshot.chain_id, address = %pending.address, "Historical candle unavailable; using live shill market cap");
                Some(snapshot.current_market_cap)
            }
            Err(error) => {
                tracing::warn!(%error, address = %pending.address, "Initial market cap unavailable");
                None
            }
        },
        Err(_) => None,
    };
    let snapshot = resolved.as_ref().expect("successful resolve handled above");
    let mut tx = pool.begin().await?;
    // Complete only the placeholder shill that triggered this task; other
    // channel calls in the same tracking period retain their own timestamps.
    sqlx::query("UPDATE shills SET initial_market_cap=$2,max_market_cap=$3,market_status=$4 WHERE id=$1")
        .bind(pending.shill_id).bind(initial).bind(snapshot.current_market_cap)
        .bind(if initial.is_some() { "tracking" } else { "unavailable" })
        .execute(&mut *tx).await?;
    sqlx::query("UPDATE tracking_periods SET highest_market_cap=GREATEST(COALESCE(highest_market_cap,0),$2) WHERE id=$1")
        .bind(pending.period_id).bind(snapshot.current_market_cap).execute(&mut *tx).await?;
    tx.commit().await?;
    // This second event is deliberately silent in the frontend: its existing
    // handler refreshes every SSE type but plays audio only for `new_shill`.
    let _ = events.send(json!({"type":"shill_updated","shill_id":pending.shill_id}).to_string());
    Ok(())
}

