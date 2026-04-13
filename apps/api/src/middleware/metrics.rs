use std::time::Instant;

use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::Request,
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

pub async fn track_requests(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let started_at = Instant::now();
    state.metrics.request_started();

    let response = next.run(request).await;
    let latency_ms = started_at.elapsed().as_millis() as u64;
    state.metrics.request_finished(response.status().as_u16(), latency_ms);

    response
}
