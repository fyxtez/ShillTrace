use anyhow::{Context, Result};
use std::{env, path::PathBuf};

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub api_bind_addr: String,
    pub frontend_origin: String,
    pub telegram_api_id: i32,
    pub telegram_api_hash: String,
    pub telegram_phone_number: String,
    pub telegram_password: String,
    pub telegram_session_path: PathBuf,
    pub photos_dir: PathBuf,
    pub market_poll_seconds: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            api_bind_addr: env::var("API_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".into()),
            frontend_origin: env::var("FRONTEND_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".into()),
            telegram_api_id: required("TELEGRAM_API_ID")?.parse().context("invalid TELEGRAM_API_ID")?,
            telegram_api_hash: required("TELEGRAM_API_HASH")?,
            telegram_phone_number: required("TELEGRAM_PHONE_NUMBER")?,
            telegram_password: env::var("TELEGRAM_PASSWORD").unwrap_or_default(),
            telegram_session_path: env::var("TELEGRAM_SESSION_PATH").unwrap_or_else(|_| "signal_ledger.session".into()).into(),
            photos_dir: env::var("PHOTOS_DIR").unwrap_or_else(|_| "storage/channel-photos".into()).into(),
            market_poll_seconds: env::var("MARKET_POLL_SECONDS").unwrap_or_else(|_| "15".into()).parse().context("invalid MARKET_POLL_SECONDS")?,
        })
    }
}

fn required(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("{name} must be set"))
}

