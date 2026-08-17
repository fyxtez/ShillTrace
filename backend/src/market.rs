use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::{cmp::Ordering, time::Duration};

const DEX_SEARCH: &str = "https://api.dexscreener.com/latest/dex/search";
const GECKO: &str = "https://api.geckoterminal.com/api/v2";

#[derive(Clone)]
pub struct MarketClient { http: Client }

#[derive(Debug, Clone)]
pub struct MarketSnapshot {
    pub chain_id: String,
    pub pair_address: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub image_url: Option<String>,
    pub website_url: Option<String>,
    pub twitter_url: Option<String>,
    pub telegram_url: Option<String>,
    pub current_market_cap: f64,
    pub current_price: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse { #[serde(default)] pairs: Vec<Pair> }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pair {
    chain_id: String,
    pair_address: String,
    base_token: Token,
    quote_token: Token,
    price_usd: Option<String>,
    market_cap: Option<f64>,
    liquidity: Option<Liquidity>,
    info: Option<PairInfo>,
}

#[derive(Deserialize)]
struct Token { address: String, symbol: Option<String>, name: Option<String> }
#[derive(Deserialize)]
struct Liquidity { usd: Option<f64> }
#[derive(Deserialize)]
struct PairInfo {
    #[serde(rename = "imageUrl")] image_url: Option<String>,
    #[serde(default)] websites: Vec<Website>,
    #[serde(default)] socials: Vec<Social>,
}
#[derive(Deserialize)]
struct Website { url: String }
#[derive(Deserialize)]
struct Social { #[serde(rename = "type")] kind: String, url: String }
#[derive(Deserialize)]
struct GeckoResponse { data: GeckoData }
#[derive(Deserialize)]
struct GeckoData { attributes: GeckoAttributes }
#[derive(Deserialize)]
struct GeckoAttributes { ohlcv_list: Vec<Vec<f64>> }

impl MarketClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            http: Client::builder()
                .timeout(Duration::from_secs(20))
                // Identify outbound market requests with the public product
                // name so provider logs no longer expose retired branding.
                .user_agent("shilltrace/0.1")
                .build()?,
        })
    }

    pub async fn resolve(&self, address: &str) -> Result<MarketSnapshot> {
        let response = self.http.get(DEX_SEARCH).query(&[("q", address)]).send().await?
            .error_for_status()?.json::<SearchResponse>().await?;
        let mut pairs: Vec<Pair> = response.pairs.into_iter().filter(|pair| {
            pair.base_token.address.eq_ignore_ascii_case(address)
                || pair.quote_token.address.eq_ignore_ascii_case(address)
        }).collect();
        pairs.sort_by(|a, b| liquidity(b).partial_cmp(&liquidity(a)).unwrap_or(Ordering::Equal));

        let pair = pairs.into_iter().find(|pair| pair.market_cap.is_some() && pair.price_usd.is_some())
            .context("DEX Screener found no pair with market-cap data")?;
        let token = if pair.base_token.address.eq_ignore_ascii_case(address) { &pair.base_token } else { &pair.quote_token };
        let current_price = pair.price_usd.as_deref().context("missing priceUsd")?.parse::<f64>()?;
        let current_market_cap = pair.market_cap.context("missing marketCap")?;
        if current_market_cap <= 0.0 || current_price <= 0.0 { bail!("non-positive market data") }

        // DEX Screener already normalizes official project links. Persisting
        // them with the token avoids extra frontend requests and lets missing
        // socials render as intentionally inactive controls.
        let info = pair.info;
        let website_url = info.as_ref().and_then(|value| value.websites.first()).map(|value| value.url.clone());
        let twitter_url = social_url(info.as_ref(), "twitter");
        let telegram_url = social_url(info.as_ref(), "telegram");
        Ok(MarketSnapshot {
            chain_id: pair.chain_id,
            pair_address: pair.pair_address,
            symbol: token.symbol.clone(),
            name: token.name.clone(),
            image_url: info.as_ref().and_then(|value| value.image_url.clone()),
            website_url,
            twitter_url,
            telegram_url,
            current_market_cap,
            current_price,
        })
    }

    pub async fn historical_market_cap(
        &self,
        snapshot: &MarketSnapshot,
        address: &str,
        at: DateTime<Utc>,
    ) -> Result<f64> {
        let network = gecko_network(&snapshot.chain_id)?;
        let token_side = self.token_side(network, &snapshot.pair_address, address).await.unwrap_or("base");
        let before = (at.timestamp() + 60).to_string();
        let url = format!("{GECKO}/networks/{network}/pools/{}/ohlcv/minute", snapshot.pair_address);
        let response = self.http.get(url).query(&[
            ("aggregate", "1"), ("before_timestamp", before.as_str()),
            ("limit", "1"), ("currency", "usd"), ("token", token_side),
        ]).send().await?.error_for_status()?.json::<GeckoResponse>().await?;
        let candle = response.data.attributes.ohlcv_list.first().context("no historical candle")?;
        let close = *candle.get(4).context("incomplete historical candle")?;
        let inferred_supply = snapshot.current_market_cap / snapshot.current_price;
        Ok(close * inferred_supply)
    }

    async fn token_side(&self, _network: &str, _pair: &str, _address: &str) -> Result<&'static str> {
        // DEX Screener normally returns the tracked asset as base token. Keeping
        // this hook separate lets us add Gecko pool metadata lookup if a quote
        // token shill appears without changing the historical calculation API.
        Ok("base")
    }
}

fn social_url(info: Option<&PairInfo>, kind: &str) -> Option<String> {
    info.and_then(|value| value.socials.iter().find(|social| social.kind.eq_ignore_ascii_case(kind)))
        .map(|social| social.url.clone())
}

fn liquidity(pair: &Pair) -> f64 {
    pair.liquidity.as_ref().and_then(|value| value.usd).unwrap_or(0.0)
}

fn gecko_network(chain: &str) -> Result<&'static str> {
    match chain.to_ascii_lowercase().as_str() {
        "ethereum" => Ok("eth"), "bsc" => Ok("bsc"), "base" => Ok("base"),
        "arbitrum" => Ok("arbitrum"), "polygon" => Ok("polygon_pos"),
        "optimism" => Ok("optimism"), "avalanche" => Ok("avax"),
        "solana" => Ok("solana"), other => bail!("historical mapping missing for {other}"),
    }
}
