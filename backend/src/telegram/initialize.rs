use crate::config::Config;
use anyhow::Result;
use grammers_client::{Client, SignInError};
use grammers_mtsender::SenderPool;
use grammers_session::{
    storages::SqliteSession,
    updates::UpdatesLike,
};
use std::{
    io,
    sync::Arc,
};
use tokio::sync::mpsc::Receiver;

// Newer grammers revisions expose SenderPool::updates as Tokio's bounded
// receiver directly, instead of the older UpdatesReceiver wrapper type.
pub struct Initialization {
    pub client: Client,
    pub updates_receiver: Receiver<UpdatesLike>,
}

pub async fn connect(config: &Config) -> Result<Initialization> {
    tracing::info!(session = %config.telegram_session_path.display(), "Opening Telegram session");
    let session_path = config
        .telegram_session_path
        .to_str()
        .expect("Telegram session path must be valid UTF-8");

    let session = SqliteSession::open(session_path).await?;

    let pool = SenderPool::new(
        Arc::new(session),
        config.telegram_api_id,
    );

    let client = Client::new(pool.handle);
    let updates_receiver = pool.updates;

    // The sender pool runner must remain active because it performs the
    // underlying Telegram network communication.
    tokio::spawn(pool.runner.run());

    if !client.is_authorized().await? {
        tracing::info!("Telegram session needs authentication; requesting OTP");
        let token = client
            .request_login_code(
                &config.telegram_phone_number,
                &config.telegram_api_hash,
            )
            .await?;

        println!("Enter Telegram OTP:");

        let mut code = String::new();
        io::stdin().read_line(&mut code)?;

        match client.sign_in(&token, code.trim()).await {
            Ok(_) => {}

            Err(SignInError::PasswordRequired(password_token)) => {
                client
                                       .check_password(
                        password_token,
                        &config.telegram_password,
                    )
                    .await?;
            }

            Err(error) => {
                return Err(error.into());
            }
        }
    }

    // Finish the async request before creating tracing's formatting values.
    // Also convert the returned name into an owned String so no non-Send
    // formatting arguments or borrowed tracing values survive in the future.
    let me = client.get_me().await?;
    let full_name = me.full_name().to_string();

    tracing::info!(
        user = full_name.as_str(),
        "Telegram connected"
    );

    Ok(Initialization {
        client,
        updates_receiver,
    })
}
