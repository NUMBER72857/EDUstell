use sqlx::{migrate::Migrator, postgres::PgPool};
use thiserror::Error;
use tracing::info;

pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, Error)]
pub enum InfrastructureError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("migration error")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

pub async fn run(pool: &PgPool) -> Result<(), InfrastructureError> {
    MIGRATOR.run(pool).await?;
    info!("database migrations applied");
    Ok(())
}
