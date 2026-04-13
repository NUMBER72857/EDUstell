use std::sync::Arc;

use application::auth::AuthError;
use application::ports::{IssuedToken, IssuedVerificationToken, TokenService};
use chrono::{DateTime, Duration, Utc};
use domain::auth::{AccessClaims, RefreshClaims, User};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub access_secret: Arc<str>,
    pub refresh_secret: Arc<str>,
    pub access_ttl_secs: i64,
    pub refresh_ttl_secs: i64,
}

#[derive(Debug, Clone)]
pub struct JwtTokenService {
    config: JwtConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmailVerificationClaims {
    sub: Uuid,
    verification_id: Uuid,
    exp: i64,
    iat: i64,
    token_type: String,
}

impl JwtTokenService {
    pub fn new(config: JwtConfig) -> Self {
        Self { config }
    }
}

impl TokenService for JwtTokenService {
    fn issue_access_token(&self, user: &User, session_id: Uuid) -> Result<IssuedToken, AuthError> {
        let now = Utc::now();
        let expires_at = now + Duration::seconds(self.config.access_ttl_secs);
        let claims = AccessClaims {
            sub: user.id,
            role: user.role,
            session_id,
            jti: Uuid::new_v4(),
            exp: expires_at.timestamp(),
            iat: now.timestamp(),
            token_type: "access".to_owned(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.access_secret.as_bytes()),
        )
        .map_err(|err| AuthError::Security(format!("failed to sign access token: {err}")))?;

        Ok(IssuedToken { token, expires_at })
    }

    fn issue_refresh_token(&self, user: &User, session_id: Uuid) -> Result<IssuedToken, AuthError> {
        let now = Utc::now();
        let expires_at = now + Duration::seconds(self.config.refresh_ttl_secs);
        let claims = RefreshClaims {
            sub: user.id,
            session_id,
            exp: expires_at.timestamp(),
            iat: now.timestamp(),
            token_type: "refresh".to_owned(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.refresh_secret.as_bytes()),
        )
        .map_err(|err| AuthError::Security(format!("failed to sign refresh token: {err}")))?;

        Ok(IssuedToken { token, expires_at })
    }

    fn issue_email_verification_token(
        &self,
        user: &User,
        verification_id: Uuid,
    ) -> Result<IssuedVerificationToken, AuthError> {
        let now = Utc::now();
        let expires_at = now + Duration::hours(24);
        let claims = EmailVerificationClaims {
            sub: user.id,
            verification_id,
            exp: expires_at.timestamp(),
            iat: now.timestamp(),
            token_type: "email_verification".to_owned(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.access_secret.as_bytes()),
        )
        .map_err(|err| {
            AuthError::Security(format!("failed to sign email verification token: {err}"))
        })?;

        Ok(IssuedVerificationToken { token, expires_at })
    }

    fn decode_access_token(&self, token: &str) -> Result<AccessClaims, AuthError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        decode::<AccessClaims>(
            token,
            &DecodingKey::from_secret(self.config.access_secret.as_bytes()),
            &validation,
        )
        .map(|data| data.claims)
        .map_err(|_| AuthError::Unauthorized)
        .and_then(|claims| {
            if claims.token_type != "access" {
                return Err(AuthError::Unauthorized);
            }
            Ok(claims)
        })
    }

    fn decode_refresh_token(&self, token: &str) -> Result<RefreshClaims, AuthError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let claims = decode::<RefreshClaims>(
            token,
            &DecodingKey::from_secret(self.config.refresh_secret.as_bytes()),
            &validation,
        )
        .map(|data| data.claims)
        .map_err(|_| AuthError::Unauthorized)?;

        if claims.token_type != "refresh" {
            return Err(AuthError::Unauthorized);
        }

        Ok(claims)
    }

    fn decode_email_verification_token(
        &self,
        token: &str,
    ) -> Result<(Uuid, Uuid, DateTime<Utc>), AuthError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let claims = decode::<EmailVerificationClaims>(
            token,
            &DecodingKey::from_secret(self.config.access_secret.as_bytes()),
            &validation,
        )
        .map(|data| data.claims)
        .map_err(|_| AuthError::InvalidVerificationToken)?;

        if claims.token_type != "email_verification" {
            return Err(AuthError::InvalidVerificationToken);
        }

        let expires_at = DateTime::<Utc>::from_timestamp(claims.exp, 0)
            .ok_or_else(|| AuthError::Security("invalid email verification exp".to_owned()))?;

        Ok((claims.sub, claims.verification_id, expires_at))
    }
}
