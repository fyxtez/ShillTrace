use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const COOLDOWN: Duration = Duration::from_secs(15 * 60);
static LAST_SENT: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

/// Sends operational failures to the same Telegram bot integration used by
/// telegram_sniper. A per-category cooldown prevents a DEX outage from
/// flooding the operator with one notification for every tracked token.
pub async fn important(category: &str, message: &str) {
    if std::env::var("TELEGRAM_BOT_TOKEN").is_err() || std::env::var("TELEGRAM_CHAT_ID").is_err() {
        tracing::warn!(
            category,
            "Telegram alert skipped: TELEGRAM_BOT_TOKEN or TELEGRAM_CHAT_ID is missing"
        );
        return;
    }

    let should_send = {
        let mut sent = LAST_SENT
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .expect("notification cooldown lock poisoned");
        match sent.get(category) {
            Some(last) if last.elapsed() < COOLDOWN => false,
            _ => {
                sent.insert(category.to_owned(), Instant::now());
                true
            }
        }
    };

    if should_send && let Err(error) = telegram_notify::send(message).await {
        tracing::error!(%error, category, "Failed to send Telegram operational alert");
    }
}
