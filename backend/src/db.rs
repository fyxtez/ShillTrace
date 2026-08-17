use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new().max_connections(10).connect(database_url).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

