use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::info;

use super::migrations::InfrastructureError;

pub type DatabasePool = PgPool;

pub async fn connect(database_url: &str) -> Result<DatabasePool, InfrastructureError> {
    let pool = PgPoolOptions::new().max_connections(10).connect(database_url).await?;

    info!("postgres pool established");

    Ok(pool)
}
