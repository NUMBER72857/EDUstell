use axum::{
    Json,
    body::Body,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tower_http::catch_panic::CatchPanicLayer;
use tracing::error;

use crate::error::ErrorEnvelope;

pub fn layer() -> CatchPanicLayer<impl Fn(Box<dyn std::any::Any + Send + 'static>) -> Response<Body> + Clone>
{
    CatchPanicLayer::custom(|panic_payload: Box<dyn std::any::Any + Send + 'static>| {
        let panic_message = panic_payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic_payload.downcast_ref::<&'static str>().map(|v| (*v).to_owned()))
            .unwrap_or_else(|| "panic without message".to_owned());

        error!(panic_message = %panic_message, "request panicked");

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorEnvelope::internal(None)),
        )
            .into_response()
    })
}
