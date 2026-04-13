use std::time::Instant;

use axum::{body::Body, http::Request, middleware::Next, response::Response};
use tracing::info;

use super::request_id::RequestContext;

pub async fn log_requests(request: Request<Body>, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let context = request.extensions().get::<RequestContext>().cloned();
    let started_at = Instant::now();

    let response = next.run(request).await;
    let latency_ms = started_at.elapsed().as_millis();

    info!(
        method = %method,
        path = %path,
        status_code = response.status().as_u16(),
        latency_ms = latency_ms as u64,
        request_id = context.as_ref().map(|ctx| ctx.request_id.as_str()).unwrap_or("unknown"),
        correlation_id = context
            .as_ref()
            .map(|ctx| ctx.correlation_id.as_str())
            .unwrap_or("unknown"),
        "request completed"
    );

    response
}
