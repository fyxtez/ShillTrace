use crate::{market::{MarketClient, MarketSnapshot}, notifications};
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use tokio::{
    sync::broadcast,
    time::{Duration, interval},
};

pub fn spawn(pool: PgPool, market: MarketClient, events: broadcast::Sender<String>, seconds: u64) {
    tokio::spawn(async move {
        let poll_seconds = seconds.max(5);
        let migration_scan_cycles = (60 / poll_seconds).max(1);
        let mut cycle = 0_u64;
        let mut timer = interval(Duration::from_secs(poll_seconds));
        loop {
            timer.tick().await;
            // Re-querying every pool once per minute detects bonding-curve/DEX
            // migrations without doubling DEX Screener traffic on every live tick.
            let scan_migrations = cycle % migration_scan_cycles == 0;
            cycle = cycle.wrapping_add(1);
            if let Err(error) = poll_once(&pool, &market, &events, scan_migrations).await {
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

async fn poll_once(
    pool: &PgPool,
    market: &MarketClient,
    events: &broadcast::Sender<String>,
    scan_migrations: bool,
) -> anyhow::Result<()> {
    let active: Vec<(i64, i64, String, String, String, Option<String>, f64, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT t.id,p.id,t.contract_address,t.chain_id,t.pair_address,t.resolved_token_address,t.current_market_cap,p.started_at FROM tokens t JOIN tracking_periods p ON p.token_id=t.id AND p.status='active' WHERE t.market_status='tracking' AND t.chain_id IS NOT NULL AND t.pair_address IS NOT NULL AND t.current_market_cap IS NOT NULL"
    ).fetch_all(pool).await?;

    // TODO:
    // HOTPATH BOTTLENECK: this loop processes active tokens one at a time —
    // each iteration awaits an HTTP call plus a full begin/4x-execute/commit
    // transaction before moving to the next token. With enough active tokens
    // (or a few slow responses), one cycle can easily exceed the poll interval.
    // Also note: tokio::time::interval defaults to MissedTickBehavior::Burst,
    // so a late cycle causes the next ticks to fire back-to-back with no gap,
    // which compounds pressure on the DEX Screener rate limit.
    // TODO: use futures::stream::iter(active).for_each_concurrent(N, ...) to
    // parallelize per-token polling (cap N to stay under the rate limit), and
    // consider interval.set_missed_tick_behavior(MissedTickBehavior::Delay).
    for (token_id, period_id, address, chain_id, pair_address, resolved_token_address, previous_market_cap, started_at) in active {
        let locked = market.resolve_locked(&chain_id, &pair_address).await;
        // A successful locked read backfills legacy rows; otherwise the persisted
        // canonical CA lets migration discovery recover after the old pool vanishes.
        let canonical_token_address = locked
            .as_ref()
            .ok()
            .map(|snapshot| snapshot.token_address.clone())
            .or(resolved_token_address);
        let migration_candidate = if scan_migrations {
            if let Some(token_address) = canonical_token_address.as_deref() {
                Some(market.resolve_preferred_on_chain(token_address, &chain_id).await)
            } else {
                None
            }
        } else {
            None
        };

        let selected: anyhow::Result<MarketSnapshot> = match locked {
            Ok(locked_snapshot) => match migration_candidate {
                        Some(Ok(candidate)) if should_migrate(&locked_snapshot, &candidate) => {
                            // A materially deeper same-chain pool is the generic
                            // signal shared by launchpad migrations on BSC, Solana,
                            // and other DEXes. Persisting it re-locks all future
                            // ticks while ignoring tiny/spoof alternative pools.
                            tracing::info!(
                                address,
                                old_pair = %locked_snapshot.pair_address,
                                new_pair = %candidate.pair_address,
                                old_liquidity = locked_snapshot.liquidity_usd,
                                new_liquidity = candidate.liquidity_usd,
                                "Detected token liquidity migration"
                            );
                            Ok(candidate)
                        }
                        Some(Err(error)) => {
                            tracing::debug!(%error, address, "Pool migration scan failed; retaining locked pair");
                            Ok(locked_snapshot)
                        }
                        _ => Ok(locked_snapshot),
                    },
            Err(locked_error) => match migration_candidate {
                Some(Ok(candidate)) if is_recovery_candidate(&candidate) => {
                    // The canonical token CA makes migration recoverable even
                    // when DEX Screener no longer returns the abandoned pool.
                    tracing::warn!(
                        address,
                        old_pair = %pair_address,
                        new_pair = %candidate.pair_address,
                        new_liquidity = candidate.liquidity_usd,
                        "Recovered tracking from unavailable migrated pool"
                    );
                    Ok(candidate)
                }
                Some(Err(scan_error)) => {
                    tracing::debug!(%scan_error, address, "Migration recovery scan failed");
                    Err(locked_error)
                }
                _ => Err(locked_error),
            },
        };

        match selected {
            Ok(snapshot) => {
                // A second guard protects Max X even if a provider returns corrupt
                // data for the locked pair itself. A 100x move between 15-second
                // polls is treated as telemetry corruption, not a real market move.
                let jump = snapshot.current_market_cap / previous_market_cap;
                if !(0.01..=100.0).contains(&jump) {
                    tracing::warn!(address, previous_market_cap, market_cap = snapshot.current_market_cap, jump, "Rejected impossible market-cap jump");
                    notifications::important(
                        "market_cap_outlier",
                        &format!("ShillTrace\nRejected market-cap outlier\nToken: {address}\nPrevious: {previous_market_cap:.2}\nReceived: {:.2}", snapshot.current_market_cap),
                    )
                    .await;
                    continue;
                }
                let now = Utc::now();
                let mut tx = pool.begin().await?;
                // Refresh socials together with market data because projects
                // frequently add official links after the first live pair.
                sqlx::query("UPDATE tokens SET current_market_cap=$2,last_market_at=$3,website_url=$4,twitter_url=$5,telegram_url=$6,pair_address=$7,resolved_token_address=$8,last_market_error=NULL,updated_at=NOW() WHERE id=$1")
                    .bind(token_id).bind(snapshot.current_market_cap).bind(now)
                    .bind(&snapshot.website_url).bind(&snapshot.twitter_url).bind(&snapshot.telegram_url)
                    .bind(&snapshot.pair_address).bind(&snapshot.token_address).execute(&mut *tx).await?;
                sqlx::query("UPDATE tracking_periods SET highest_market_cap=GREATEST(COALESCE(highest_market_cap,0),$2) WHERE id=$1")
                    .bind(period_id).bind(snapshot.current_market_cap).execute(&mut *tx).await?;
                sqlx::query("UPDATE shills SET max_market_cap=GREATEST(COALESCE(max_market_cap,0),$2) WHERE tracking_period_id=$1")
                    .bind(period_id).bind(snapshot.current_market_cap).execute(&mut *tx).await?;
                // Repair only extremely impossible recent initial values left by
                // the former base-only Gecko lookup. The tight time window and
                // 100x threshold avoid rewriting legitimate historical calls.
                sqlx::query("UPDATE shills SET initial_market_cap=$2 WHERE tracking_period_id=$1 AND shilled_at>=NOW()-INTERVAL '30 minutes' AND initial_market_cap IS NOT NULL AND (initial_market_cap>$2*100 OR initial_market_cap<$2/100)")
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
                let bucket = if age <= 604_800 {
                    60
                } else if age <= 2_592_000 {
                    300
                } else {
                    3_600
                };
                let sampled_at =
                    chrono::DateTime::from_timestamp((now.timestamp() / bucket) * bucket, 0)
                        .unwrap_or(now);
                sqlx::query("INSERT INTO market_cap_samples(token_id,tracking_period_id,market_cap,recorded_at) VALUES($1,$2,$3,$4) ON CONFLICT(token_id,tracking_period_id,recorded_at) DO UPDATE SET market_cap=EXCLUDED.market_cap")
                    .bind(token_id).bind(period_id).bind(snapshot.current_market_cap).bind(sampled_at).execute(&mut *tx).await?;
                tx.commit().await?;
                let _ = events.send(json!({"type":"market_update","token_id":token_id,"market_cap":snapshot.current_market_cap}).to_string());
            }
            Err(error) => {
                // Missing data remains informational; only the user can stop a
                // token, so a temporary DEX outage never silently removes it.
                sqlx::query("UPDATE tokens SET last_market_error=$2,updated_at=NOW() WHERE id=$1")
                    .bind(token_id)
                    .bind(error.to_string())
                    .execute(pool)
                    .await?;
                let detail = error.to_string();
                let category = if detail.contains("429") {
                    "dexscreener_rate_limit"
                } else {
                    "dexscreener_market_error"
                };
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

fn should_migrate(
    current: &MarketSnapshot,
    candidate: &MarketSnapshot,
) -> bool {
    if current
        .pair_address
        .eq_ignore_ascii_case(&candidate.pair_address)
    {
        return false;
    }
    // Requiring at least $1k and 50% more depth prevents ordinary secondary
    // pools from stealing tracking while still recognizing abandoned launchpad
    // pools whose liquidity becomes zero or unavailable after migration.
    candidate.liquidity_usd >= 1_000.0
        && candidate.liquidity_usd > current.liquidity_usd * 1.5
}

fn is_recovery_candidate(candidate: &MarketSnapshot) -> bool {
    // With no surviving old pool to compare, meaningful liquidity is required
    // before recovery re-locks tracking to a newly discovered pair.
    candidate.liquidity_usd >= 1_000.0
}
