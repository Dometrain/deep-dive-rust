use crate::error::AppError;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use tracing::instrument;

pub type DbPool = Arc<PgPool>;

// skip_all: never record the connection string, it contains credentials.
#[instrument(skip_all)]
pub async fn create_pool(database_url: &str) -> Result<DbPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(5 * 60))
        .max_lifetime(Duration::from_secs(30 * 60))
        .connect(database_url)
        .await?;

    Ok(Arc::new(pool))
}

#[instrument(skip(pool))]
pub async fn initialize_database(pool: &DbPool) -> Result<(), AppError> {
    sqlx::migrate!("./migrations").run(pool.as_ref()).await?;
    Ok(())
}
