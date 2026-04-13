use std::sync::Arc;

use axum::{Json, extract::State};
use serde::Serialize;

use crate::{
    api::{ResponseEnvelope, ok},
    error::ApiError,
    state::AppState,
};

#[derive(Debug, Serialize)]
pub struct HealthData {
    pub status: &'static str,
    pub service: String,
    pub environment: &'static str,
    pub uptime_seconds: u64,
}

pub async fn health(State(state): State<Arc<AppState>>) -> Json<ResponseEnvelope<HealthData>> {
    state.metrics.health_check();
    ok(HealthData {
        status: "ok",
        service: state.config.app_name.clone(),
        environment: state.config.environment.as_str(),
        uptime_seconds: state.started_at.elapsed().as_secs(),
    })
}

pub async fn ready(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ResponseEnvelope<HealthData>>, ApiError> {
    state.metrics.health_check();
    sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&state.db_pool).await?;

    Ok(ok(HealthData {
        status: "ready",
        service: state.config.app_name.clone(),
        environment: state.config.environment.as_str(),
        uptime_seconds: state.started_at.elapsed().as_secs(),
    }))
}
