use application::auth::AuthError;
use domain::auth::{AuthenticatedUser, UserRole};

use crate::error::ApiError;

pub fn require_any_role(user: &AuthenticatedUser, allowed: &[UserRole]) -> Result<(), ApiError> {
    if allowed.iter().any(|role| role == &user.role) {
        return Ok(());
    }

    Err(ApiError::from(AuthError::Forbidden))
}
