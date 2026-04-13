use std::sync::Arc;

use application::auth::{self, LoginInput, RegisterInput, mfa_hooks};
use axum::{Json, extract::State};
use domain::auth::UserRole;
use serde::{Deserialize, Serialize};

use crate::{
    api::{Meta, ResponseEnvelope, Validate, ValidatedJson, ok, ok_with_meta},
    error::ApiError,
    extractors::current_user::CurrentUser,
    middleware::auth::require_any_role,
    state::AppState,
};

use super::dto::UserDto;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub role: String,
}

impl Validate for RegisterRequest {
    fn validate(&self) -> Result<(), ApiError> {
        let email = self.email.trim();
        if email.is_empty() || !email.contains('@') || email.len() > 254 {
            return Err(ApiError::validation_with_field("valid email is required", "email"));
        }
        if self.password.len() < 12 || self.password.len() > 128 {
            return Err(ApiError::validation_with_field(
                "password must be between 12 and 128 characters",
                "password",
            ));
        }
        if self.role.trim().is_empty() {
            return Err(ApiError::validation_with_field("role is required", "role"));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

impl Validate for LoginRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.email.trim().is_empty() || self.email.len() > 254 {
            return Err(ApiError::validation_with_field("email is required", "email"));
        }
        if self.password.is_empty() || self.password.len() > 128 {
            return Err(ApiError::validation_with_field("password is required", "password"));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

impl Validate for RefreshRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.refresh_token.trim().is_empty() || self.refresh_token.len() > 4096 {
            return Err(ApiError::validation_with_field(
                "refresh_token is required",
                "refresh_token",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

impl Validate for LogoutRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.refresh_token.trim().is_empty() || self.refresh_token.len() > 4096 {
            return Err(ApiError::validation_with_field(
                "refresh_token is required",
                "refresh_token",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

impl Validate for VerifyEmailRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.token.trim().is_empty() || self.token.len() > 4096 {
            return Err(ApiError::validation_with_field("token is required", "token"));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: chrono::DateTime<chrono::Utc>,
    pub refresh_expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct MfaHooksResponse {
    pub required: bool,
    pub methods: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponseData {
    pub user: UserDto,
    pub tokens: TokenPair,
    pub email_verification_required: bool,
    pub mfa: MfaHooksResponse,
}

#[derive(Debug, Serialize)]
pub struct EmailVerificationResponseData {
    pub user: UserDto,
    pub verification_token: Option<String>,
    pub verification_expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct LogoutResponseData {
    pub success: bool,
}

#[derive(Debug, Serialize)]
pub struct ProtectedPing {
    pub ok: bool,
    pub role: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    ValidatedJson(request): ValidatedJson<RegisterRequest>,
) -> Result<Json<ResponseEnvelope<AuthResponseData>>, ApiError> {
    let role = request
        .role
        .parse::<UserRole>()
        .map_err(|_| ApiError::validation("invalid role supplied"))?;

    let result = auth::register(
        &state.auth_repo,
        &state.password_hasher,
        &state.token_service,
        &state.audit_repo,
        RegisterInput { email: request.email, password: request.password, role },
    )
    .await?;
    let mfa = mfa_hooks(&result.user);

    Ok(ok(AuthResponseData {
        user: result.user.clone().into(),
        tokens: TokenPair {
            access_token: result.tokens.access_token,
            refresh_token: result.tokens.refresh_token,
            access_expires_at: result.tokens.access_expires_at,
            refresh_expires_at: result.tokens.refresh_expires_at,
        },
        email_verification_required: !result.user.email_verified,
        mfa: MfaHooksResponse { required: mfa.required, methods: mfa.methods },
    }))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    ValidatedJson(request): ValidatedJson<LoginRequest>,
) -> Result<Json<ResponseEnvelope<AuthResponseData>>, ApiError> {
    let result = auth::login(
        &state.auth_repo,
        &state.password_hasher,
        &state.token_service,
        LoginInput { email: request.email, password: request.password },
    )
    .await?;
    let mfa = mfa_hooks(&result.user);

    Ok(ok(AuthResponseData {
        user: result.user.clone().into(),
        tokens: TokenPair {
            access_token: result.tokens.access_token,
            refresh_token: result.tokens.refresh_token,
            access_expires_at: result.tokens.access_expires_at,
            refresh_expires_at: result.tokens.refresh_expires_at,
        },
        email_verification_required: !result.user.email_verified,
        mfa: MfaHooksResponse { required: mfa.required, methods: mfa.methods },
    }))
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    ValidatedJson(request): ValidatedJson<RefreshRequest>,
) -> Result<Json<ResponseEnvelope<AuthResponseData>>, ApiError> {
    let result = auth::refresh(
        &state.auth_repo,
        &state.password_hasher,
        &state.token_service,
        &request.refresh_token,
    )
    .await?;
    let mfa = mfa_hooks(&result.user);

    Ok(ok(AuthResponseData {
        user: result.user.clone().into(),
        tokens: TokenPair {
            access_token: result.tokens.access_token,
            refresh_token: result.tokens.refresh_token,
            access_expires_at: result.tokens.access_expires_at,
            refresh_expires_at: result.tokens.refresh_expires_at,
        },
        email_verification_required: !result.user.email_verified,
        mfa: MfaHooksResponse { required: mfa.required, methods: mfa.methods },
    }))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    ValidatedJson(request): ValidatedJson<LogoutRequest>,
) -> Result<Json<ResponseEnvelope<LogoutResponseData>>, ApiError> {
    auth::logout(&state.auth_repo, &state.token_service, &request.refresh_token).await?;

    Ok(ok(LogoutResponseData { success: true }))
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<UserDto>>, ApiError> {
    let user = auth::current_user(&state.auth_repo, current_user.0.user_id).await?;

    Ok(ok(user.into()))
}

pub async fn begin_email_verification(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<EmailVerificationResponseData>>, ApiError> {
    let result = auth::begin_email_verification(
        &state.auth_repo,
        &state.password_hasher,
        &state.token_service,
        current_user.0.user_id,
    )
    .await?;

    Ok(ok_with_meta(
        EmailVerificationResponseData {
            user: result.user.into(),
            verification_token: if state.config.environment.as_str() == "local" {
                Some(result.verification_token)
            } else {
                None
            },
            verification_expires_at: result.verification_expires_at,
        },
        Meta {
            filters: Some(serde_json::json!({
                "delivery": "scaffold_only",
                "token_returned_in_response": state.config.environment.as_str() == "local"
            })),
            ..Meta::default()
        },
    ))
}

pub async fn verify_email(
    State(state): State<Arc<AppState>>,
    ValidatedJson(request): ValidatedJson<VerifyEmailRequest>,
) -> Result<Json<ResponseEnvelope<UserDto>>, ApiError> {
    let user = auth::verify_email(
        &state.auth_repo,
        &state.password_hasher,
        &state.token_service,
        &request.token,
    )
    .await?;

    Ok(ok(user.into()))
}

pub async fn platform_admin_ping(
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<ProtectedPing>>, ApiError> {
    require_any_role(&current_user.0, &[UserRole::PlatformAdmin])?;

    Ok(ok(ProtectedPing { ok: true, role: current_user.0.role.as_str().to_owned() }))
}
