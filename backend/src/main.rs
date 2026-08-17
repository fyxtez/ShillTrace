mod api;
mod config;
mod db;
mod detection;
mod market;
mod models;
mod notifications;
mod telegram;
mod tracker;

use anyhow::Context;
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Default to useful INFO logs when RUST_LOG is not configured so a local
    // run always shows database, Telegram, channel-sync and HTTP progress.
    let log_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(log_filter).init();

    let config = config::Config::from_env()?;
    tracing::info!("Connecting to PostgreSQL");
    let pool = db::connect(&config.database_url).await?;
    tracing::info!("PostgreSQL connected and migrations completed");
    let market = market::MarketClient::new()?;
    let (events, _) = broadcast::channel(256);

    let state = api::AppState {
        pool: pool.clone(),
        market: market.clone(),
        events: events.clone(),
        // Expose the configured live refresh cadence to the UI instead of
        // hard-coding the default 15 seconds in chart copy.
        market_poll_seconds: config.market_poll_seconds,
    };

    tracker::spawn(
        pool.clone(),
        market,
        events.clone(),
        config.market_poll_seconds,
    );

    let telegram_config = config.clone();
    let telegram_pool = pool.clone();
    let telegram_events = events.clone();

    // Run Telegram ingestion independently from the HTTP server so incoming
    // channel messages continue being processed while Axum handles requests.
    tokio::spawn(async move {
        if let Err(error) = telegram::run(telegram_config, telegram_pool, telegram_events).await {
            tracing::error!(
                %error,
                "telegram ingestion stopped"
            );
            // A stopped ingestion task means future shills would be missed, so
            // this failure must reach the operator outside the local terminal.
            notifications::important(
                "telegram_ingestion_stopped",
                &format!("ShillTrace\nTelegram ingestion stopped\nError: {error}"),
            )
            .await;
        }
    });

    let app = api::router(state, &config.frontend_origin)
        .nest_service("/photos", ServeDir::new(&config.photos_dir));

    let listener = tokio::net::TcpListener::bind(&config.api_bind_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.api_bind_addr))?;

    tracing::info!(
        address = %config.api_bind_addr,
        "ShillTrace API listening"
    );

    axum::serve(listener, app).await?;

    Ok(())
}
