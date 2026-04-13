use chrono::{DateTime, Utc};
use domain::auth::{EmailVerificationToken, NewUser, PublicUser, RefreshSession, User, UserRole};
use uuid::Uuid;

use crate::ports::{AuthRepository, PasswordHasher, TokenService};
use crate::{
    audit::{AuditEvent, AuditService},
    repos::AuditLogRepository,
};

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("email already exists")]
    EmailAlreadyExists,
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("email already verified")]
    EmailAlreadyVerified,
    #[error("invalid verification token")]
    InvalidVerificationToken,
    #[error("repository error: {0}")]
    Repository(String),
    #[error("security error: {0}")]
    Security(String),
}

#[derive(Debug, Clone)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
    pub role: UserRole,
}

#[derive(Debug, Clone)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AuthResponse {
    pub user: PublicUser,
    pub tokens: AuthTokens,
}

#[derive(Debug, Clone)]
pub struct EmailVerificationResponse {
    pub user: PublicUser,
    pub verification_token: String,
    pub verification_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MfaHooks {
    pub required: bool,
    pub methods: Vec<&'static str>,
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn ensure_registerable_role(role: UserRole) -> Result<(), AuthError> {
    if matches!(role, UserRole::PlatformAdmin | UserRole::SchoolAdmin) {
        return Err(AuthError::Forbidden);
    }

    Ok(())
}

fn ensure_active_user(user: &User) -> Result<(), AuthError> {
    if user.status == "active" {
        return Ok(());
    }

    Err(AuthError::Unauthorized)
}

pub async fn register<R, H, T, A>(
    repo: &R,
    hasher: &H,
    tokens: &T,
    audits: &A,
    input: RegisterInput,
) -> Result<AuthResponse, AuthError>
where
    R: AuthRepository,
    H: PasswordHasher,
    T: TokenService,
    A: AuditLogRepository,
{
    ensure_registerable_role(input.role)?;
    let normalized_email = normalize_email(&input.email);

    if repo.find_user_by_email(&normalized_email).await?.is_some() {
        return Err(AuthError::EmailAlreadyExists);
    }

    let password_hash = hasher.hash_secret(&input.password).await?;
    let user = repo
        .create_user(NewUser { email: normalized_email.clone(), password_hash, role: input.role })
        .await?;

    AuditService::new(audits)
        .record(AuditEvent::user_registered(user.id, user.role.as_str(), &normalized_email))
        .await
        .map_err(|error| AuthError::Repository(error.to_string()))?;

    issue_session(repo, hasher, tokens, user).await
}

pub async fn login<R, H, T>(
    repo: &R,
    hasher: &H,
    tokens: &T,
    input: LoginInput,
) -> Result<AuthResponse, AuthError>
where
    R: AuthRepository,
    H: PasswordHasher,
    T: TokenService,
{
    let user = repo
        .find_user_by_email(&normalize_email(&input.email))
        .await?
        .ok_or(AuthError::InvalidCredentials)?;
    ensure_active_user(&user)?;

    let password_ok = hasher.verify_secret(&input.password, &user.password_hash).await?;
    if !password_ok {
        return Err(AuthError::InvalidCredentials);
    }

    issue_session(repo, hasher, tokens, user).await
}

pub async fn refresh<R, H, T>(
    repo: &R,
    hasher: &H,
    tokens: &T,
    refresh_token: &str,
) -> Result<AuthResponse, AuthError>
where
    R: AuthRepository,
    H: PasswordHasher,
    T: TokenService,
{
    let claims = tokens.decode_refresh_token(refresh_token)?;
    let existing_session =
        repo.find_refresh_session(claims.session_id).await?.ok_or(AuthError::Unauthorized)?;

    if existing_session.user_id != claims.sub || existing_session.revoked_at.is_some() {
        return Err(AuthError::Unauthorized);
    }

    if existing_session.expires_at <= Utc::now() {
        return Err(AuthError::Unauthorized);
    }

    let matches = hasher.verify_secret(refresh_token, &existing_session.token_hash).await?;
    if !matches {
        return Err(AuthError::Unauthorized);
    }

    repo.revoke_refresh_session(existing_session.id).await?;

    let user = repo.find_user_by_id(claims.sub).await?.ok_or(AuthError::Unauthorized)?;
    ensure_active_user(&user)?;

    issue_session(repo, hasher, tokens, user).await
}

pub async fn logout<R, T>(repo: &R, tokens: &T, refresh_token: &str) -> Result<(), AuthError>
where
    R: AuthRepository,
    T: TokenService,
{
    let claims = tokens.decode_refresh_token(refresh_token)?;
    repo.revoke_refresh_session(claims.session_id).await?;
    Ok(())
}

pub async fn current_user<R>(repo: &R, user_id: Uuid) -> Result<PublicUser, AuthError>
where
    R: AuthRepository,
{
    let user = repo.find_user_by_id(user_id).await?.ok_or(AuthError::NotFound)?;
    ensure_active_user(&user)?;
    Ok(user.into())
}

pub async fn begin_email_verification<R, H, T>(
    repo: &R,
    hasher: &H,
    tokens: &T,
    user_id: Uuid,
) -> Result<EmailVerificationResponse, AuthError>
where
    R: AuthRepository,
    H: PasswordHasher,
    T: TokenService,
{
    let user = repo.find_user_by_id(user_id).await?.ok_or(AuthError::NotFound)?;
    ensure_active_user(&user)?;
    if user.email_verified {
        return Err(AuthError::EmailAlreadyVerified);
    }

    let verification_id = Uuid::new_v4();
    let issued = tokens.issue_email_verification_token(&user, verification_id)?;
    let token_hash = hasher.hash_secret(&issued.token).await?;

    repo.store_email_verification_token(EmailVerificationToken {
        id: verification_id,
        user_id: user.id,
        token_hash,
        expires_at: issued.expires_at,
        consumed_at: None,
    })
    .await?;

    Ok(EmailVerificationResponse {
        user: user.into(),
        verification_token: issued.token,
        verification_expires_at: issued.expires_at,
    })
}

pub async fn verify_email<R, H, T>(
    repo: &R,
    hasher: &H,
    tokens: &T,
    token: &str,
) -> Result<PublicUser, AuthError>
where
    R: AuthRepository,
    H: PasswordHasher,
    T: TokenService,
{
    let (user_id, verification_id, expires_at) = tokens.decode_email_verification_token(token)?;
    if expires_at <= Utc::now() {
        return Err(AuthError::InvalidVerificationToken);
    }

    let record = repo
        .find_email_verification_token(verification_id)
        .await?
        .ok_or(AuthError::InvalidVerificationToken)?;

    if record.user_id != user_id || record.consumed_at.is_some() || record.expires_at <= Utc::now()
    {
        return Err(AuthError::InvalidVerificationToken);
    }

    let matches = hasher.verify_secret(token, &record.token_hash).await?;
    if !matches {
        return Err(AuthError::InvalidVerificationToken);
    }

    repo.consume_email_verification_token(record.id).await?;
    repo.mark_email_verified(user_id).await?;

    let user = repo.find_user_by_id(user_id).await?.ok_or(AuthError::NotFound)?;
    ensure_active_user(&user)?;
    Ok(user.into())
}

pub fn mfa_hooks(user: &PublicUser) -> MfaHooks {
    MfaHooks {
        required: user.mfa_enabled,
        methods: if user.mfa_enabled { vec!["totp"] } else { vec![] },
    }
}

async fn issue_session<R, H, T>(
    repo: &R,
    hasher: &H,
    tokens: &T,
    user: User,
) -> Result<AuthResponse, AuthError>
where
    R: AuthRepository,
    H: PasswordHasher,
    T: TokenService,
{
    let session_id = Uuid::new_v4();
    let access = tokens.issue_access_token(&user, session_id)?;
    let refresh = tokens.issue_refresh_token(&user, session_id)?;
    let refresh_token_hash = hasher.hash_secret(&refresh.token).await?;

    repo.store_refresh_session(RefreshSession {
        id: session_id,
        user_id: user.id,
        token_hash: refresh_token_hash,
        expires_at: refresh.expires_at,
        revoked_at: None,
    })
    .await?;

    Ok(AuthResponse {
        user: user.into(),
        tokens: AuthTokens {
            access_token: access.token,
            refresh_token: refresh.token,
            access_expires_at: access.expires_at,
            refresh_expires_at: refresh.expires_at,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use chrono::{Duration, Utc};
    use domain::auth::{
        AccessClaims, EmailVerificationToken, NewUser, RefreshClaims, RefreshSession, User,
        UserRole,
    };

    use crate::ports::{
        AuthRepository, IssuedToken, IssuedVerificationToken, PasswordHasher, TokenService,
    };
    use crate::repos::{AuditLogRepository, PersistenceError};

    use super::*;

    #[derive(Default, Clone)]
    struct MemoryRepo {
        users: Arc<Mutex<HashMap<Uuid, User>>>,
        users_by_email: Arc<Mutex<HashMap<String, Uuid>>>,
        refresh_sessions: Arc<Mutex<HashMap<Uuid, RefreshSession>>>,
        verification_tokens: Arc<Mutex<HashMap<Uuid, EmailVerificationToken>>>,
    }

    #[async_trait]
    impl AuthRepository for MemoryRepo {
        async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, AuthError> {
            let users_by_email = self.users_by_email.lock().unwrap();
            let users = self.users.lock().unwrap();
            Ok(users_by_email.get(email).and_then(|id| users.get(id)).cloned())
        }
        async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<User>, AuthError> {
            Ok(self.users.lock().unwrap().get(&user_id).cloned())
        }
        async fn create_user(&self, new_user: NewUser) -> Result<User, AuthError> {
            if self.users_by_email.lock().unwrap().contains_key(&new_user.email) {
                return Err(AuthError::EmailAlreadyExists);
            }
            let user = User {
                id: Uuid::new_v4(),
                email: new_user.email.clone(),
                password_hash: new_user.password_hash,
                role: new_user.role,
                email_verified: false,
                mfa_enabled: false,
                status: "active".to_owned(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            self.users_by_email.lock().unwrap().insert(new_user.email, user.id);
            self.users.lock().unwrap().insert(user.id, user.clone());
            Ok(user)
        }
        async fn store_refresh_session(&self, session: RefreshSession) -> Result<(), AuthError> {
            self.refresh_sessions.lock().unwrap().insert(session.id, session);
            Ok(())
        }
        async fn find_refresh_session(
            &self,
            session_id: Uuid,
        ) -> Result<Option<RefreshSession>, AuthError> {
            Ok(self.refresh_sessions.lock().unwrap().get(&session_id).cloned())
        }
        async fn revoke_refresh_session(&self, session_id: Uuid) -> Result<(), AuthError> {
            if let Some(session) = self.refresh_sessions.lock().unwrap().get_mut(&session_id) {
                session.revoked_at = Some(Utc::now());
            }
            Ok(())
        }
        async fn store_email_verification_token(
            &self,
            token: EmailVerificationToken,
        ) -> Result<(), AuthError> {
            self.verification_tokens.lock().unwrap().insert(token.id, token);
            Ok(())
        }
        async fn find_email_verification_token(
            &self,
            token_id: Uuid,
        ) -> Result<Option<EmailVerificationToken>, AuthError> {
            Ok(self.verification_tokens.lock().unwrap().get(&token_id).cloned())
        }
        async fn consume_email_verification_token(&self, token_id: Uuid) -> Result<(), AuthError> {
            if let Some(token) = self.verification_tokens.lock().unwrap().get_mut(&token_id) {
                token.consumed_at = Some(Utc::now());
            }
            Ok(())
        }
        async fn mark_email_verified(&self, user_id: Uuid) -> Result<(), AuthError> {
            if let Some(user) = self.users.lock().unwrap().get_mut(&user_id) {
                user.email_verified = true;
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct TestHasher;

    #[async_trait]
    impl PasswordHasher for TestHasher {
        async fn hash_secret(&self, value: &str) -> Result<String, AuthError> {
            Ok(format!("hashed::{value}"))
        }
        async fn verify_secret(&self, value: &str, hash: &str) -> Result<bool, AuthError> {
            Ok(hash == format!("hashed::{value}"))
        }
    }

    #[derive(Default)]
    struct TestTokens;

    #[derive(Default)]
    struct FakeAuditRepo {
        items: Arc<Mutex<Vec<domain::persistence::AuditLog>>>,
    }

    #[async_trait]
    impl AuditLogRepository for FakeAuditRepo {
        async fn append(
            &self,
            audit_log: domain::persistence::AuditLog,
        ) -> Result<domain::persistence::AuditLog, PersistenceError> {
            self.items.lock().unwrap().push(audit_log.clone());
            Ok(audit_log)
        }

        async fn list_by_entity(
            &self,
            entity_type: &str,
            entity_id: Uuid,
        ) -> Result<Vec<domain::persistence::AuditLog>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .iter()
                .filter(|item| item.entity_type == entity_type && item.entity_id == Some(entity_id))
                .cloned()
                .collect())
        }

        async fn list_by_actor(
            &self,
            actor_user_id: Uuid,
        ) -> Result<Vec<domain::persistence::AuditLog>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .iter()
                .filter(|item| item.actor_user_id == Some(actor_user_id))
                .cloned()
                .collect())
        }
    }

    impl TokenService for TestTokens {
        fn issue_access_token(
            &self,
            user: &User,
            session_id: Uuid,
        ) -> Result<IssuedToken, AuthError> {
            Ok(IssuedToken {
                token: format!("access:{}:{session_id}", user.id),
                expires_at: Utc::now() + Duration::minutes(15),
            })
        }
        fn issue_refresh_token(
            &self,
            user: &User,
            session_id: Uuid,
        ) -> Result<IssuedToken, AuthError> {
            Ok(IssuedToken {
                token: format!("refresh:{}:{session_id}", user.id),
                expires_at: Utc::now() + Duration::days(30),
            })
        }
        fn issue_email_verification_token(
            &self,
            user: &User,
            verification_id: Uuid,
        ) -> Result<IssuedVerificationToken, AuthError> {
            Ok(IssuedVerificationToken {
                token: format!("verify:{}:{verification_id}", user.id),
                expires_at: Utc::now() + Duration::hours(24),
            })
        }
        fn decode_access_token(&self, token: &str) -> Result<AccessClaims, AuthError> {
            let (_, user_id, session_id) = parse_three_part_token(token, "access")?;
            Ok(AccessClaims {
                sub: user_id,
                role: UserRole::Parent,
                session_id,
                jti: Uuid::new_v4(),
                exp: (Utc::now() + Duration::minutes(15)).timestamp(),
                iat: Utc::now().timestamp(),
                token_type: "access".to_owned(),
            })
        }
        fn decode_refresh_token(&self, token: &str) -> Result<RefreshClaims, AuthError> {
            let (_, user_id, session_id) = parse_three_part_token(token, "refresh")?;
            Ok(RefreshClaims {
                sub: user_id,
                session_id,
                exp: (Utc::now() + Duration::days(30)).timestamp(),
                iat: Utc::now().timestamp(),
                token_type: "refresh".to_owned(),
            })
        }
        fn decode_email_verification_token(
            &self,
            token: &str,
        ) -> Result<(Uuid, Uuid, DateTime<Utc>), AuthError> {
            let (_, user_id, verification_id) = parse_three_part_token(token, "verify")?;
            Ok((user_id, verification_id, Utc::now() + Duration::hours(24)))
        }
    }

    fn parse_three_part_token(
        token: &str,
        expected_prefix: &str,
    ) -> Result<(String, Uuid, Uuid), AuthError> {
        let parts: Vec<_> = token.split(':').collect();
        if parts.len() != 3 || parts[0] != expected_prefix {
            return Err(AuthError::Unauthorized);
        }
        let user_id = parts[1].parse().map_err(|_| AuthError::Unauthorized)?;
        let token_id = parts[2].parse().map_err(|_| AuthError::Unauthorized)?;
        Ok((parts[0].to_owned(), user_id, token_id))
    }

    #[tokio::test]
    async fn register_creates_user_and_session() {
        let repo = MemoryRepo::default();
        let hasher = TestHasher;
        let tokens = TestTokens;
        let audits = FakeAuditRepo::default();

        let result = register(
            &repo,
            &hasher,
            &tokens,
            &audits,
            RegisterInput {
                email: "parent@example.com".to_owned(),
                password: "correct horse battery staple".to_owned(),
                role: UserRole::Parent,
            },
        )
        .await
        .unwrap();

        assert_eq!(result.user.email, "parent@example.com");
        assert_eq!(result.user.role, UserRole::Parent);
        assert!(!result.user.email_verified);
        assert!(!result.user.mfa_enabled);
        assert_eq!(audits.items.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn login_rejects_invalid_password() {
        let repo = MemoryRepo::default();
        let hasher = TestHasher;
        let tokens = TestTokens;

        let _ = repo
            .create_user(NewUser {
                email: "parent@example.com".to_owned(),
                password_hash: "hashed::right-password".to_owned(),
                role: UserRole::Parent,
            })
            .await
            .unwrap();

        let error = login(
            &repo,
            &hasher,
            &tokens,
            LoginInput {
                email: "parent@example.com".to_owned(),
                password: "wrong-password".to_owned(),
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, AuthError::InvalidCredentials));
    }

    #[tokio::test]
    async fn refresh_rotates_session() {
        let repo = MemoryRepo::default();
        let hasher = TestHasher;
        let tokens = TestTokens;
        let audits = FakeAuditRepo::default();

        let registered = register(
            &repo,
            &hasher,
            &tokens,
            &audits,
            RegisterInput {
                email: "parent@example.com".to_owned(),
                password: "correct horse battery staple".to_owned(),
                role: UserRole::Parent,
            },
        )
        .await
        .unwrap();

        let refreshed =
            refresh(&repo, &hasher, &tokens, &registered.tokens.refresh_token).await.unwrap();

        assert_eq!(refreshed.user.email, "parent@example.com");
        assert_ne!(refreshed.tokens.refresh_token, registered.tokens.refresh_token);
    }

    #[tokio::test]
    async fn email_verification_marks_user_verified() {
        let repo = MemoryRepo::default();
        let hasher = TestHasher;
        let tokens = TestTokens;
        let audits = FakeAuditRepo::default();

        let registered = register(
            &repo,
            &hasher,
            &tokens,
            &audits,
            RegisterInput {
                email: "parent@example.com".to_owned(),
                password: "correct horse battery staple".to_owned(),
                role: UserRole::Parent,
            },
        )
        .await
        .unwrap();

        let verification =
            begin_email_verification(&repo, &hasher, &tokens, registered.user.id).await.unwrap();
        let verified =
            verify_email(&repo, &hasher, &tokens, &verification.verification_token).await.unwrap();

        assert!(verified.email_verified);
    }
}
