use std::{env, sync::OnceLock};

use sqlx::{
    Executor, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use tokio::sync::{Mutex, MutexGuard};

use crate::db::migrations;

pub struct TestDatabase {
    pool: PgPool,
    _guard: MutexGuard<'static, ()>,
}

impl TestDatabase {
    pub async fn isolated() -> Result<Option<Self>, sqlx::Error> {
        let Some(database_url) = configured_test_database_url() else {
            return Ok(None);
        };
        let guard = test_lock().lock().await;
        let pool = PgPoolOptions::new().max_connections(1).connect(&database_url).await?;
        migrations::run(&pool).await.expect("migrations must succeed for tests");
        truncate_all(&pool).await?;
        Ok(Some(Self { pool, _guard: guard }))
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub fn configured_test_database_url() -> Option<String> {
    env::var("TEST_DATABASE_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

pub async fn truncate_all(pool: &PgPool) -> Result<(), sqlx::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"
        SELECT tablename
        FROM pg_tables
        WHERE schemaname = current_schema()
          AND tablename <> '_sqlx_migrations'
        ORDER BY tablename
        "#,
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    let joined = rows.iter().map(|name| format!("\"{name}\"")).collect::<Vec<_>>().join(", ");
    let statement = format!("TRUNCATE TABLE {joined} RESTART IDENTITY CASCADE");
    pool.execute(statement.as_str()).await?;
    Ok(())
}

pub fn connect_options_for_test() -> PgConnectOptions {
    configured_test_database_url()
        .expect("TEST_DATABASE_URL or DATABASE_URL must be set for database-backed tests")
        .parse::<PgConnectOptions>()
        .expect("test database url must be a valid postgres url")
}
