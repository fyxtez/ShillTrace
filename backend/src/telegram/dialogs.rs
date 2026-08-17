use grammers_client::{
    Client,
    peer::{Dialog, Peer},
};
use sqlx::PgPool;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub async fn sync_channels(
    client: &Client,
    pool: &PgPool,
    photos_dir: &Path,
) -> anyhow::Result<usize> {
    tokio::fs::create_dir_all(photos_dir).await?;
    tracing::info!(directory = %photos_dir.display(), "Synchronizing Telegram dialogs and channel photos");
    let mut dialogs = client.iter_dialogs();
    let mut count = 0;
    while let Some(dialog) = dialogs.next().await? {
        let Some(id) = dialog.peer.id().bare_id() else {
            continue;
        };
        let kind = match &dialog.peer {
            Peer::Channel(_) => "channel",
            Peer::Group(_) => "group",
            Peer::User(_) => "user",
            _ => "other",
        };
        let name = dialog.peer.name().unwrap_or("Unnamed");
        sqlx::query("INSERT INTO channels(telegram_id,name,kind) VALUES($1,$2,$3) ON CONFLICT(telegram_id) DO UPDATE SET name=EXCLUDED.name,kind=EXCLUDED.kind,updated_at=NOW()")
            .bind(id).bind(name).bind(kind).execute(pool).await?;

        if kind == "channel" {
            let path = photos_dir.join(format!("{id}.jpg"));
            // Report whether a photo was reused, downloaded, or unavailable so
            // startup activity never looks like a silent application freeze.
            let has_photo = if path.exists() {
                tracing::debug!(
                    channel_id = id,
                    channel = name,
                    "Using cached channel photo"
                );
                true
            } else if download_photo(client, &dialog, &path).await {
                tracing::info!(channel_id = id, channel = name, "Downloaded channel photo");
                true
            } else {
                tracing::debug!(channel_id = id, channel = name, "Channel photo unavailable");
                false
            };
            sqlx::query("UPDATE channels SET has_photo=$2 WHERE telegram_id=$1")
                .bind(id)
                .bind(has_photo)
                .execute(pool)
                .await?;
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        count += 1;
        if count % 25 == 0 {
            tracing::info!(dialogs = count, "Telegram dialog sync progress");
        }
    }
    Ok(count)
}

async fn download_photo(client: &Client, dialog: &Dialog, path: &PathBuf) -> bool {
    let Ok(Some(photo)) = dialog.peer.photo(true).await else {
        return false;
    };
    client.download_media(&photo, path).await.is_ok()
}
