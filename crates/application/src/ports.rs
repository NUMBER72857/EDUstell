use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::auth::{
    AccessClaims, EmailVerificationToken, NewUser, RefreshClaims, RefreshSession, User,
};
use uuid::Uuid;

use crate::auth::AuthError;

pub struct IssuedToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

pub struct IssuedVerificationToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait AuthRepository: Send + Sync {
    async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, AuthError>;
    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<User>, AuthError>;
    async fn create_user(&self, new_user: NewUser) -> Result<User, AuthError>;
    async fn store_refresh_session(&self, session: RefreshSession) -> Result<(), AuthError>;
    async fn find_refresh_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<RefreshSession>, AuthError>;
    async fn revoke_refresh_session(&self, session_id: Uuid) -> Result<(), AuthError>;
    async fn store_email_verification_token(
        &self,
        token: EmailVerificationToken,
    ) -> Result<(), AuthError>;
    async fn find_email_verification_token(
        &self,
        token_id: Uuid,
    ) -> Result<Option<EmailVerificationToken>, AuthError>;
    async fn consume_email_verification_token(&self, token_id: Uuid) -> Result<(), AuthError>;
    async fn mark_email_verified(&self, user_id: Uuid) -> Result<(), AuthError>;
}

#[async_trait]
pub trait PasswordHasher: Send + Sync {
    async fn hash_secret(&self, value: &str) -> Result<String, AuthError>;
    async fn verify_secret(&self, value: &str, hash: &str) -> Result<bool, AuthError>;
}

pub trait TokenService: Send + Sync {
    fn issue_access_token(&self, user: &User, session_id: Uuid) -> Result<IssuedToken, AuthError>;
    fn issue_refresh_token(&self, user: &User, session_id: Uuid) -> Result<IssuedToken, AuthError>;
    fn issue_email_verification_token(
        &self,
        user: &User,
        verification_id: Uuid,
    ) -> Result<IssuedVerificationToken, AuthError>;
    fn decode_access_token(&self, token: &str) -> Result<AccessClaims, AuthError>;
    fn decode_refresh_token(&self, token: &str) -> Result<RefreshClaims, AuthError>;
    fn decode_email_verification_token(
        &self,
        token: &str,
    ) -> Result<(Uuid, Uuid, DateTime<Utc>), AuthError>;
}
