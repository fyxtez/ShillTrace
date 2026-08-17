use crate::{
    market::MarketClient,
    models::{ChannelView, ShillView},
    notifications,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode},
    response::{
        IntoResponse, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{delete, get, patch, post},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{convert::Infallible, time::Duration};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub market: MarketClient,
    pub events: broadcast::Sender<String>,
    pub market_poll_seconds: u64,
}

pub fn router(state: AppState, frontend_origin: &str) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(
            frontend_origin
                .parse::<HeaderValue>()
                .expect("valid FRONTEND_ORIGIN"),
        )
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);
    Router::new()
        .route("/api/health", get(health))
        .route("/api/shills", get(list_shills))
        .route("/api/shills/{id}/seen", post(mark_seen))
        .route("/api/tokens/{id}", delete(remove_token))
        .route("/api/tokens/{id}/retry", post(retry_token))
        .route("/api/shills/{id}/history", get(shill_history))
        .route("/api/channels", get(list_channels))
        .route("/api/channels/{id}/ignored", patch(set_ignored))
        .route("/api/channels/{id}/pinned", patch(set_pinned))
        .route("/api/channels/{id}/hidden", patch(set_hidden))
        .route("/api/events", get(events))
        .layer(cors)
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"status":"ok","market_poll_seconds":state.market_poll_seconds}))
}

#[derive(Deserialize)]
struct ShillQuery {
    unseen: Option<bool>,
    channel_id: Option<i64>,
    include_removed: Option<bool>,
}

async fn list_shills(
    State(state): State<AppState>,
    Query(query): Query<ShillQuery>,
) -> Result<Json<Vec<ShillView>>, ApiError> {
    let rows = sqlx::query_as::<_, ShillView>(r#"
        SELECT s.id,s.token_id,t.symbol,t.name AS token_name,t.contract_address,t.chain_id,t.image_url,t.pair_address,
               t.website_url,t.twitter_url,t.telegram_url,
               s.channel_id,c.name AS channel_name,c.has_photo AS channel_has_photo,m.body AS message,
               s.shilled_at,s.initial_market_cap,t.current_market_cap,s.max_market_cap,s.market_status,
               s.seen_at,(SELECT COUNT(*) FROM shills sx WHERE sx.token_id=s.token_id) AS shill_count
        FROM shills s
        JOIN tracking_periods p ON p.id=s.tracking_period_id
        JOIN tokens t ON t.id=s.token_id
        JOIN channels c ON c.telegram_id=s.channel_id
        JOIN telegram_messages m ON m.id=s.first_message_id
        WHERE ($1::BOOLEAN IS NULL OR $1=FALSE OR s.seen_at IS NULL)
          AND ($2::BIGINT IS NULL OR s.channel_id=$2)
          AND ($3::BOOLEAN=TRUE OR p.status='active')
        ORDER BY s.shilled_at DESC
    "#).bind(query.unseen).bind(query.channel_id).bind(query.include_removed.unwrap_or(false)).fetch_all(&state.pool).await?;
    Ok(Json(rows))
}

async fn mark_seen(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let changed = sqlx::query("UPDATE shills SET seen_at=COALESCE(seen_at,NOW()) WHERE id=$1")
        .bind(id)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(ApiError::not_found("shill not found"));
    }
    let _ = state
        .events
        .send(json!({"type":"shill_seen","shill_id":id}).to_string());
    Ok(Json(json!({"ok":true})))
}

async fn remove_token(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let changed = sqlx::query("UPDATE tracking_periods SET status='removed',stopped_at=NOW() WHERE token_id=$1 AND status='active'")
        .bind(id).execute(&state.pool).await?.rows_affected();
    if changed == 0 {
        return Err(ApiError::not_found("active token not found"));
    }
    let _ = state
        .events
        .send(json!({"type":"token_removed","token_id":id}).to_string());
    Ok(Json(json!({"ok":true})))
}

async fn retry_token(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT contract_address FROM tokens WHERE id=$1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    let Some((address,)) = row else {
        return Err(ApiError::not_found("token not found"));
    };
    let snapshot = match state.market.resolve(&address).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let detail = error.to_string();
            let category = if detail.contains("429") {
                "dexscreener_rate_limit"
            } else {
                "manual_market_retry"
            };
            notifications::important(
                category,
                &format!(
                    "ShillTrace\nManual market retry failed\nToken: {address}\nError: {detail}"
                ),
            )
            .await;
            return Err(ApiError::market(error));
        }
    };
    sqlx::query("UPDATE tokens SET chain_id=$2,pair_address=$3,symbol=$4,name=$5,image_url=$6,current_market_cap=$7,website_url=$8,twitter_url=$9,telegram_url=$10,market_status='tracking',last_market_error=NULL,last_market_at=NOW(),updated_at=NOW() WHERE id=$1")
        .bind(id).bind(&snapshot.chain_id).bind(&snapshot.pair_address).bind(&snapshot.symbol)
        .bind(&snapshot.name).bind(&snapshot.image_url).bind(snapshot.current_market_cap)
        .bind(&snapshot.website_url).bind(&snapshot.twitter_url).bind(&snapshot.telegram_url).execute(&state.pool).await?;

    let unavailable: Vec<(i64, i64, DateTime<Utc>)> = sqlx::query_as("SELECT id,tracking_period_id,shilled_at FROM shills WHERE token_id=$1 AND market_status='unavailable'")
        .bind(id).fetch_all(&state.pool).await?;
    for (shill_id, tracking_period_id, shilled_at) in unavailable {
        // Robinhood and other newly-added chains may not yet exist in
        // GeckoTerminal. A manual retry for a recent live shill can safely use
        // the current DEX Screener market cap as the closest timestamp sample.
        let historical = state
            .market
            .historical_market_cap(&snapshot, &address, shilled_at)
            .await;
        // The earliest stored sample was captured immediately after ingestion
        // and is the most accurate fallback for chains without candle history.
        let earliest_sample: Option<f64> = sqlx::query_scalar("SELECT market_cap FROM market_cap_samples WHERE token_id=$1 AND tracking_period_id=$2 ORDER BY recorded_at LIMIT 1")
            .bind(id).bind(tracking_period_id).fetch_optional(&state.pool).await?;
        let initial = historical.ok().or(earliest_sample).or_else(|| {
            (Utc::now().signed_duration_since(shilled_at).num_minutes() <= 5)
                .then_some(snapshot.current_market_cap)
        });
        if let Some(initial) = initial {
            sqlx::query("UPDATE shills SET initial_market_cap=$2,max_market_cap=$3,market_status='tracking' WHERE id=$1")
                .bind(shill_id).bind(initial).bind(snapshot.current_market_cap).execute(&state.pool).await?;
        }
    }
    let _ = state
        .events
        .send(json!({"type":"token_retry_succeeded","token_id":id}).to_string());
    Ok(Json(json!({"ok":true})))
}

async fn shill_history(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    // History belongs to one shill/tracking period, not every lifetime period
    // of a token. The synthetic first point guarantees the graph starts at the
    // exact Initial MC and Telegram shill timestamp selected by the user.
    let anchor: Option<(i64, DateTime<Utc>, Option<f64>)> = sqlx::query_as(
        "SELECT tracking_period_id,shilled_at,initial_market_cap FROM shills WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((tracking_period_id, shilled_at, initial_market_cap)) = anchor else {
        return Err(ApiError::not_found("shill not found"));
    };
    let samples: Vec<(DateTime<Utc>, f64)> = sqlx::query_as(
        "SELECT recorded_at,market_cap FROM market_cap_samples WHERE tracking_period_id=$1 AND recorded_at>$2 ORDER BY recorded_at"
    ).bind(tracking_period_id).bind(shilled_at).fetch_all(&state.pool).await?;
    let mut history = Vec::with_capacity(samples.len() + 1);
    if let Some(initial) = initial_market_cap {
        history.push(json!({"time":shilled_at,"market_cap":initial,"is_initial":true}));
    }
    history.extend(
        samples.into_iter().map(
            |(time, market_cap)| json!({"time":time,"market_cap":market_cap,"is_initial":false}),
        ),
    );
    Ok(Json(json!(history)))
}

async fn list_channels(State(state): State<AppState>) -> Result<Json<Vec<ChannelView>>, ApiError> {
    let rows = sqlx::query_as::<_, ChannelView>(r#"
        SELECT c.telegram_id,c.name,c.kind,c.is_ignored,c.is_pinned,c.is_hidden,c.has_photo,COUNT(s.id) AS shill_count,
               MAX(s.shilled_at) AS last_shill_at,
               AVG(t.current_market_cap/NULLIF(s.initial_market_cap,0)) AS average_current_x,
               PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY t.current_market_cap/NULLIF(s.initial_market_cap,0)) AS median_current_x,
               AVG(s.max_market_cap/NULLIF(s.initial_market_cap,0)) AS average_max_x
        FROM channels c LEFT JOIN shills s ON s.channel_id=c.telegram_id LEFT JOIN tokens t ON t.id=s.token_id
        WHERE c.kind='channel' GROUP BY c.telegram_id ORDER BY c.name
    "#).fetch_all(&state.pool).await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
struct IgnoredBody {
    ignored: bool,
}

async fn set_ignored(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<IgnoredBody>,
) -> Result<Json<Value>, ApiError> {
    // Ignored channels cannot remain invisibly pinned; re-enabling monitoring
    // returns them to All Channels until the user pins them again.
    // Returning a channel to monitoring also clears its ignored-list-only
    // hidden flag, so ignoring it again later never makes it vanish silently.
    let changed = sqlx::query("UPDATE channels SET is_ignored=$2,is_pinned=CASE WHEN $2 THEN FALSE ELSE is_pinned END,is_hidden=CASE WHEN $2 THEN is_hidden ELSE FALSE END,updated_at=NOW() WHERE telegram_id=$1 AND kind='channel'")
        .bind(id).bind(body.ignored).execute(&state.pool).await?.rows_affected();
    if changed == 0 {
        return Err(ApiError::not_found("channel not found"));
    }
    Ok(Json(json!({"ok":true})))
}

#[derive(Deserialize)]
struct PinnedBody {
    pinned: bool,
}

async fn set_pinned(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<PinnedBody>,
) -> Result<Json<Value>, ApiError> {
    // Pinning affects presentation only; monitoring and historical data remain
    // unchanged, so channels can move between sections without side effects.
    let changed = sqlx::query("UPDATE channels SET is_pinned=$2,updated_at=NOW() WHERE telegram_id=$1 AND kind='channel' AND is_ignored=FALSE")
        .bind(id).bind(body.pinned).execute(&state.pool).await?.rows_affected();
    if changed == 0 {
        return Err(ApiError::not_found("monitored channel not found"));
    }
    Ok(Json(json!({"ok":true})))
}

#[derive(Deserialize)]
struct HiddenBody {
    hidden: bool,
}

async fn set_hidden(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<HiddenBody>,
) -> Result<Json<Value>, ApiError> {
    // Hiding is deliberately limited to ignored channels: it keeps the large
    // discovery list tidy without ever concealing a channel being monitored.
    let changed = sqlx::query("UPDATE channels SET is_hidden=$2,updated_at=NOW() WHERE telegram_id=$1 AND kind='channel' AND is_ignored=TRUE")
        .bind(id).bind(body.hidden).execute(&state.pool).await?.rows_affected();
    if changed == 0 {
        return Err(ApiError::not_found("ignored channel not found"));
    }
    Ok(Json(json!({"ok":true})))
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>> {
    let mut receiver = state.events.subscribe();
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(data) => yield Ok(Event::default().data(data)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

struct ApiError {
    status: StatusCode,
    message: String,
}
impl ApiError {
    fn not_found(message: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
    fn market(error: anyhow::Error) -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            message: error.to_string(),
        }
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(json!({"error":self.message}))).into_response()
    }
}
