use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::{cmp::Ordering, time::Duration};

const DEX_SEARCH: &str = "https://api.dexscreener.com/latest/dex/search";
const DEX_PAIRS: &str = "https://api.dexscreener.com/latest/dex/pairs";
const DEX_TOKEN_PAIRS: &str = "https://api.dexscreener.com/token-pairs/v1";
const GECKO: &str = "https://api.geckoterminal.com/api/v2";

#[derive(Clone)]
pub struct MarketClient {
    http: Client,
}

#[derive(Debug, Clone)]
pub struct MarketSnapshot {
    pub chain_id: String,
    pub pair_address: String,
    pub token_address: String,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub image_url: Option<String>,
    pub website_url: Option<String>,
    pub twitter_url: Option<String>,
    pub telegram_url: Option<String>,
    pub current_market_cap: f64,
    pub current_price: f64,
    pub liquidity_usd: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResponse {
    #[serde(default)]
    pairs: Vec<Pair>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pair {
    chain_id: String,
    pair_address: String,
    base_token: Token,
    price_usd: Option<String>,
    market_cap: Option<f64>,
    liquidity: Option<Liquidity>,
    info: Option<PairInfo>,
}

#[derive(Deserialize)]
struct Token {
    address: String,
    symbol: Option<String>,
    name: Option<String>,
}
#[derive(Deserialize)]
struct Liquidity {
    usd: Option<f64>,
}
#[derive(Deserialize)]
struct PairInfo {
    #[serde(rename = "imageUrl")]
    image_url: Option<String>,
    #[serde(default)]
    websites: Vec<Website>,
    #[serde(default)]
    socials: Vec<Social>,
}
#[derive(Deserialize)]
struct Website {
    url: String,
}
#[derive(Deserialize)]
struct Social {
    #[serde(rename = "type")]
    kind: String,
    url: String,
}
#[derive(Deserialize)]
struct GeckoResponse {
    data: GeckoData,
}
#[derive(Deserialize)]
struct GeckoData {
    attributes: GeckoAttributes,
}
#[derive(Deserialize)]
struct GeckoAttributes {
    ohlcv_list: Vec<Vec<f64>>,
}
#[derive(Deserialize)]
struct GeckoPoolResponse {
    data: GeckoPoolData,
}
#[derive(Deserialize)]
struct GeckoPoolData {
    relationships: GeckoPoolRelationships,
}
#[derive(Deserialize)]
struct GeckoPoolRelationships {
    base_token: GeckoRelationship,
    quote_token: GeckoRelationship,
}
#[derive(Deserialize)]
struct GeckoRelationship {
    data: GeckoRelationshipData,
}
#[derive(Deserialize)]
struct GeckoRelationshipData {
    id: String,
}


#[derive(Deserialize)]
struct JsonRpcResponse {
    result: Option<String>,
}

#[derive(Deserialize)]
struct SolanaRpcResponse {
    result: Option<SolanaAccountResult>,
}

#[derive(Deserialize)]
struct SolanaAccountResult {
    value: Option<SolanaAccountValue>,
}

#[derive(Deserialize)]
struct SolanaAccountValue {
    owner: String,
}

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

    // NOTE: no client-side rate limiting/backoff here. On 429 we only log +
    // notify (with a 15-min per-category cooldown) and keep polling at the
    // same cadence next cycle — combined with the tracker's burst-catchup
    // behavior this can turn into a self-sustaining loop of 429s as the
    // number of tracked tokens grows.
    pub async fn resolve(&self, address: &str) -> Result<MarketSnapshot> {
        let response = self
            .http
            .get(DEX_SEARCH)
            .query(&[("q", address)])
            .send()
            .await?
            .error_for_status()?
            .json::<SearchResponse>()
            .await?;
        // Telegram callers sometimes post the DEX pair address instead of the
        // base-token CA. Accept an exact pair-address match as a safe fallback,
        // while sorting direct base-token matches first so an address that is
        // both a token and appears in unrelated search results stays canonical.
        let mut pairs: Vec<Pair> = response
            .pairs
            .into_iter()
            .filter(|pair| pair_matches_address(pair, address))
            .collect();
        pairs.sort_by(|a, b| {
            base_token_match(b, address)
                .cmp(&base_token_match(a, address))
                .then_with(|| {
                    liquidity(b)
                        .partial_cmp(&liquidity(a))
                        .unwrap_or(Ordering::Equal)
                })
        });

        let pair = pairs
            .into_iter()
            .find(|pair| pair.market_cap.is_some() && pair.price_usd.is_some())
            .context("DEX Screener found no pair with market-cap data")?;
        // DEX Screener already normalizes official project links. Persisting
        // them with the token avoids extra frontend requests and lets missing
        // socials render as intentionally inactive controls.
        snapshot_from_pair(pair)
    }

    pub async fn resolve_preferred_on_chain(
        &self,
        token_address: &str,
        chain_id: &str,
    ) -> Result<MarketSnapshot> {
        let url = format!("{DEX_TOKEN_PAIRS}/{chain_id}/{token_address}");
        let response = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<Pair>>()
            .await?;
        // The token-pairs endpoint is authoritative for every pool of one token;
        // unlike ranked search results it cannot omit a migration candidate.
        // Keep the same-chain/base-token checks as defense in depth because
        // pair-level price and market cap describe the base side of the pool.
        let mut pairs: Vec<Pair> = response
            .into_iter()
            .filter(|pair| {
                pair.chain_id.eq_ignore_ascii_case(chain_id)
                    && base_token_match(pair, token_address)
                    && pair.market_cap.is_some()
                    && pair.price_usd.is_some()
            })
            .collect();
        pairs.sort_by(|a, b| {
            liquidity(b)
                .partial_cmp(&liquidity(a))
                .unwrap_or(Ordering::Equal)
        });
        snapshot_from_pair(
            pairs
                .into_iter()
                .next()
                .context("DEX Screener found no same-chain token pair with market-cap data")?,
        )
    }

    // Tracking must stay on the exact DEX pair chosen during enrichment. Re-running
    // the broad search every 15 seconds can jump to a spoof/malformed pool for the
    // same mint and permanently inflate Max X with market caps the real chart never had.
    pub async fn resolve_locked(
        &self,
        chain_id: &str,
        pair_address: &str,
    ) -> Result<MarketSnapshot> {
        let url = format!("{DEX_PAIRS}/{chain_id}/{pair_address}");
        let response = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<SearchResponse>()
            .await?;
        let pair = response
            .pairs
            .into_iter()
            .find(|pair| {
                pair.pair_address.eq_ignore_ascii_case(pair_address)
                    && pair.market_cap.is_some()
                    && pair.price_usd.is_some()
            })
            .context("locked DEX pair no longer has market-cap data")?;
        snapshot_from_pair(pair)
    }

    // For a raw 0x address there is no trustworthy chain hint. After DEX
    // resolution has failed, probe supported EVM chains in the background and
    // classify the address as a wallet only when the account has real on-chain
    // activity (nonce or native balance) and no contract bytecode on that chain.
    // Requiring activity is important: eth_getCode returns 0x for both EOAs and
    // completely nonexistent addresses, which would otherwise turn every unknown
    // contract into a false wallet on some other EVM chain.
    pub async fn discover_evm_wallet_chain(&self, address: &str) -> Option<&'static str> {
        const CHAINS: [&str; 6] = [
            "ethereum", "base", "bsc", "arbitrum", "polygon", "optimism",
        ];

        for chain in CHAINS {
            match self.is_active_evm_wallet(address, chain).await {
                Ok(true) => return Some(chain),
                Ok(false) => {}
                Err(error) => tracing::debug!(%error, address, chain, "EVM wallet discovery probe failed"),
            }
        }
        None
    }

    async fn is_active_evm_wallet(&self, address: &str, chain: &str) -> Result<bool> {
        let endpoint = evm_rpc(chain)?;
        let code = self.evm_rpc_string(endpoint, "eth_getCode", address).await?;
        if !is_zero_hex(&code) {
            return Ok(false);
        }

        let nonce = self.evm_rpc_string(endpoint, "eth_getTransactionCount", address).await?;
        if !is_zero_hex(&nonce) {
            return Ok(true);
        }

        let balance = self.evm_rpc_string(endpoint, "eth_getBalance", address).await?;
        Ok(!is_zero_hex(&balance))
    }

    async fn evm_rpc_string(&self, endpoint: &str, method: &str, address: &str) -> Result<String> {
        let params = serde_json::json!([address, "latest"]);
        let response = self
            .http
            .post(endpoint)
            .json(&serde_json::json!({
                "jsonrpc":"2.0",
                "id":1,
                "method":method,
                "params":params
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<JsonRpcResponse>()
            .await?;
        response.result.context("EVM RPC returned no result")
    }

    // Wallet classification deliberately runs only after token resolution fails,
    // so the Telegram hot path remains DB-only. EVM EOAs are verified by bytecode
    // and Solana wallets by their System Program owner; an unresolved token is never
    // moved merely because DEX Screener has not indexed it yet.
    pub async fn is_wallet(&self, address: &str, chain_hint: &str) -> Result<bool> {
        match chain_hint {
            "ethereum" | "base" | "bsc" | "arbitrum" | "polygon" | "optimism" => {
                let endpoint = evm_rpc(chain_hint)?;
                let response = self
                    .http
                    .post(endpoint)
                    .json(&serde_json::json!({
                        "jsonrpc":"2.0",
                        "id":1,
                        "method":"eth_getCode",
                        "params":[address,"latest"]
                    }))
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<JsonRpcResponse>()
                    .await?;
                Ok(matches!(response.result.as_deref(), Some("0x") | Some("0x0")))
            }
            "solana" => {
                let response = self
                    .http
                    .post("https://solana-rpc.publicnode.com")
                    .json(&serde_json::json!({
                        "jsonrpc":"2.0",
                        "id":1,
                        "method":"getAccountInfo",
                        "params":[address,{"encoding":"base64"}]
                    }))
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<SolanaRpcResponse>()
                    .await?;
                Ok(response
                    .result
                    .and_then(|result| result.value)
                    .is_some_and(|value| value.owner == "11111111111111111111111111111111"))
            }
            _ => Ok(false),
        }
    }

    pub async fn historical_market_cap(
        &self,
        snapshot: &MarketSnapshot,
        address: &str,
        at: DateTime<Utc>,
    ) -> Result<f64> {
        let network = gecko_network(&snapshot.chain_id)?;
        // DEX search can return a pool where the tracked contract is the quote
        // token. Gecko must chart that exact side or its price is multiplied by
        // an unrelated supply and produces impossible historical market caps.
        let token_side = self
            .token_side(network, &snapshot.pair_address, address)
            .await?;
        let before = (at.timestamp() + 60).to_string();
        let url = format!(
            "{GECKO}/networks/{network}/pools/{}/ohlcv/minute",
            snapshot.pair_address
        );
        let response = self
            .http
            .get(url)
            .query(&[
                ("aggregate", "1"),
                ("before_timestamp", before.as_str()),
                ("limit", "1"),
                ("currency", "usd"),
                ("token", token_side),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<GeckoResponse>()
            .await?;
        let candle = response
            .data
            .attributes
            .ohlcv_list
            .first()
            .context("no historical candle")?;
        let close = *candle.get(4).context("incomplete historical candle")?;
        let inferred_supply = snapshot.current_market_cap / snapshot.current_price;
        let historical = close * inferred_supply;
        // New Telegram updates are resolved almost immediately. A historical
        // value thousands of times away from the live MC therefore indicates a
        // provider/pool mismatch, not real price movement, and must not persist.
        if Utc::now().signed_duration_since(at).num_minutes().abs() <= 10 {
            let ratio = historical / snapshot.current_market_cap;
            if !(0.1..=10.0).contains(&ratio) {
                bail!("historical market cap failed recent-call sanity check: ratio {ratio:.2}x")
            }
        }
        Ok(historical)
    }

    async fn token_side(&self, network: &str, pair: &str, address: &str) -> Result<&'static str> {
        let url = format!("{GECKO}/networks/{network}/pools/{pair}");
        let pool = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json::<GeckoPoolResponse>()
            .await?;
        let tracked = address.to_ascii_lowercase();
        let base = pool
            .data
            .relationships
            .base_token
            .data
            .id
            .to_ascii_lowercase();
        let quote = pool
            .data
            .relationships
            .quote_token
            .data
            .id
            .to_ascii_lowercase();
        if base.ends_with(&tracked) {
            Ok("base")
        } else if quote.ends_with(&tracked) {
            Ok("quote")
        } else {
            bail!("tracked token is not part of the selected Gecko pool")
        }
    }
}

fn snapshot_from_pair(pair: Pair) -> Result<MarketSnapshot> {
    let current_price = pair
        .price_usd
        .as_deref()
        .context("missing priceUsd")?
        .parse::<f64>()?;
    let current_market_cap = pair.market_cap.context("missing marketCap")?;
    if current_market_cap <= 0.0 || current_price <= 0.0 {
        bail!("non-positive market data")
    }
    let token = &pair.base_token;
    let info = pair.info;
    let website_url = info
        .as_ref()
        .and_then(|value| value.websites.first())
        .map(|value| value.url.clone());
    let twitter_url = social_url(info.as_ref(), "twitter");
    let telegram_url = social_url(info.as_ref(), "telegram");
    Ok(MarketSnapshot {
        chain_id: pair.chain_id,
        pair_address: pair.pair_address,
        token_address: token.address.clone(),
        symbol: token.symbol.clone(),
        name: token.name.clone(),
        image_url: info.as_ref().and_then(|value| value.image_url.clone()),
        website_url,
        twitter_url,
        telegram_url,
        current_market_cap,
        current_price,
        liquidity_usd: pair
            .liquidity
            .as_ref()
            .and_then(|value| value.usd)
            .unwrap_or(0.0),
    })
}

fn is_zero_hex(value: &str) -> bool {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    digits.is_empty() || digits.bytes().all(|byte| byte == b'0')
}

fn evm_rpc(chain: &str) -> Result<&'static str> {
    match chain {
        "ethereum" => Ok("https://ethereum-rpc.publicnode.com"),
        "base" => Ok("https://base-rpc.publicnode.com"),
        "bsc" => Ok("https://bsc-rpc.publicnode.com"),
        "arbitrum" => Ok("https://arbitrum-one-rpc.publicnode.com"),
        "polygon" => Ok("https://polygon-bor-rpc.publicnode.com"),
        "optimism" => Ok("https://optimism-rpc.publicnode.com"),
        other => bail!("wallet RPC mapping missing for {other}"),
    }
}

fn social_url(info: Option<&PairInfo>, kind: &str) -> Option<String> {
    info.and_then(|value| {
        value
            .socials
            .iter()
            .find(|social| social.kind.eq_ignore_ascii_case(kind))
    })
    .map(|social| social.url.clone())
}

fn liquidity(pair: &Pair) -> f64 {
    pair.liquidity
        .as_ref()
        .and_then(|value| value.usd)
        .unwrap_or(0.0)
}

fn base_token_match(pair: &Pair, address: &str) -> bool {
    pair.base_token.address.eq_ignore_ascii_case(address)
}

fn pair_matches_address(pair: &Pair, address: &str) -> bool {
    base_token_match(pair, address) || pair.pair_address.eq_ignore_ascii_case(address)
}

fn gecko_network(chain: &str) -> Result<&'static str> {
    match chain.to_ascii_lowercase().as_str() {
        "ethereum" => Ok("eth"),
        "bsc" => Ok("bsc"),
        "base" => Ok("base"),
        "arbitrum" => Ok("arbitrum"),
        "polygon" => Ok("polygon_pos"),
        "optimism" => Ok("optimism"),
        "avalanche" => Ok("avax"),
        "solana" => Ok("solana"),
        // GeckoTerminal identifies the TRON network as `tron`; mapping it here
        // enables timestamp-aligned initial market caps for TRX/TRC-20 shills.
        "tron" => Ok("tron"),
        other => bail!("historical mapping missing for {other}"),
    }
}
