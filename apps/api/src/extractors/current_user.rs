use std::sync::Arc;

use application::ports::TokenService;
use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use domain::auth::AuthenticatedUser;

use crate::{error::ApiError, state::AppState};

pub struct CurrentUser(pub AuthenticatedUser);

impl FromRequestParts<Arc<AppState>> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| ApiError::from(application::auth::AuthError::Unauthorized))?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| ApiError::from(application::auth::AuthError::Unauthorized))?;

        let claims = state.token_service.decode_access_token(token)?;

        Ok(Self(AuthenticatedUser {
            user_id: claims.sub,
            role: claims.role,
            session_id: claims.session_id,
        }))
    }
}
