use std::sync::Arc;

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};

use crate::{error::ApiError, middleware::request_id::RequestContext, state::AppState};

pub struct CurrentRequestContext(pub RequestContext);

impl FromRequestParts<Arc<AppState>> for CurrentRequestContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<RequestContext>()
            .cloned()
            .map(Self)
            .ok_or_else(|| ApiError::internal("request context unavailable"))
    }
}
