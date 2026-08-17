use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ShillView {
    pub id: i64,
    pub token_id: i64,
    pub symbol: Option<String>,
    pub token_name: Option<String>,
    pub contract_address: String,
    pub chain_id: Option<String>,
    pub image_url: Option<String>,
    pub pair_address: Option<String>,
    pub website_url: Option<String>,
    pub twitter_url: Option<String>,
    pub telegram_url: Option<String>,
    pub channel_id: i64,
    pub channel_name: String,
    pub channel_has_photo: bool,
    pub message: String,
    pub shilled_at: DateTime<Utc>,
    pub initial_market_cap: Option<f64>,
    pub current_market_cap: Option<f64>,
    pub max_market_cap: Option<f64>,
    pub market_status: String,
    pub seen_at: Option<DateTime<Utc>>,
    pub shill_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ChannelView {
    pub telegram_id: i64,
    pub name: String,
    pub kind: String,
    pub is_ignored: bool,
    pub is_pinned: bool,
    pub has_photo: bool,
    pub shill_count: i64,
    pub last_shill_at: Option<DateTime<Utc>>,
    pub average_current_x: Option<f64>,
    pub median_current_x: Option<f64>,
    pub average_max_x: Option<f64>,
}
