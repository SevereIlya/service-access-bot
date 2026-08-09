use crate::application::error::AppResult;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

pub async fn create_pg_pool(database_url: &str) -> AppResult<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_mins(10))
        .max_lifetime(Duration::from_mins(30))
        .connect(database_url)
        .await?;
    Ok(pool)
}
