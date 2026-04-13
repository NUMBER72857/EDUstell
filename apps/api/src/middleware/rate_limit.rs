use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    body::Body,
    http::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::error::ApiError;

#[derive(Clone, Copy)]
struct Window {
    started_at: Instant,
    hits: u32,
}

static AUTH_RATE_LIMITS: OnceLock<Mutex<HashMap<String, Window>>> = OnceLock::new();

const AUTH_WINDOW: Duration = Duration::from_secs(60);
const AUTH_MAX_REQUESTS: u32 = 20;

pub async fn auth_rate_limit(request: Request<Body>, next: Next) -> Response {
    let path = request.uri().path();
    if !path.starts_with("/api/v1/auth/")
        || !matches!(
            path,
            "/api/v1/auth/register"
                | "/api/v1/auth/login"
                | "/api/v1/auth/refresh"
                | "/api/v1/auth/verify-email"
                | "/api/v1/auth/verify-email/begin"
        )
    {
        return next.run(request).await;
    }

    let key = request
        .headers()
        .get("x-forwarded-for")
        .or_else(|| request.headers().get("x-real-ip"))
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "local".to_owned());

    let now = Instant::now();
    let limiter = AUTH_RATE_LIMITS.get_or_init(|| Mutex::new(HashMap::new()));
    let limited = {
        let mut store = match limiter.lock() {
            Ok(store) => store,
            Err(_) => return ApiError::internal("auth rate limiter unavailable").into_response(),
        };
        let entry = store.entry(key).or_insert(Window { started_at: now, hits: 0 });

        if now.duration_since(entry.started_at) > AUTH_WINDOW {
            entry.started_at = now;
            entry.hits = 0;
        }

        entry.hits += 1;
        entry.hits > AUTH_MAX_REQUESTS
    };

    if limited {
        return (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "60")],
            axum::Json(serde_json::json!({
                "error": {
                    "code": shared::error_codes::ErrorCode::Validation.as_str(),
                    "message": "auth rate limit exceeded"
                }
            })),
        )
            .into_response();
    }

    next.run(request).await
}
