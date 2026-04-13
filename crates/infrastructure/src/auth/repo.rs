use application::auth::AuthError;
use application::ports::AuthRepository;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::auth::{EmailVerificationToken, NewUser, RefreshSession, User, UserRole};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PgAuthRepository {
    pool: PgPool,
}

impl PgAuthRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct UserRow {
    id: Uuid,
    email: String,
    password_hash: String,
    role: String,
    email_verified: bool,
    mfa_enabled: bool,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct RefreshSessionRow {
    id: Uuid,
    user_id: Uuid,
    token_hash: String,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct EmailVerificationTokenRow {
    id: Uuid,
    user_id: Uuid,
    token_hash: String,
    expires_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
}

#[async_trait]
impl AuthRepository for PgAuthRepository {
    async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, email, password_hash, role, email_verified, status, created_at, updated_at
            , mfa_enabled
            FROM users
            WHERE lower(email) = lower($1)
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_repo_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<User>, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, email, password_hash, role, email_verified, status, created_at, updated_at
            , mfa_enabled
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_repo_error)?;

        row.map(TryInto::try_into).transpose()
    }

    async fn create_user(&self, new_user: NewUser) -> Result<User, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO users (email, password_hash, role, email_verified, mfa_enabled, status)
            VALUES ($1, $2, $3, FALSE, FALSE, 'active')
            RETURNING id, email, password_hash, role, email_verified, mfa_enabled, status, created_at, updated_at
            "#,
        )
        .bind(new_user.email)
        .bind(new_user.password_hash)
        .bind(new_user.role.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(map_repo_error)?;

        row.try_into()
    }

    async fn store_refresh_session(&self, session: RefreshSession) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at, revoked_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(session.token_hash)
        .bind(session.expires_at)
        .bind(session.revoked_at)
        .execute(&self.pool)
        .await
        .map_err(map_repo_error)?;

        Ok(())
    }

    async fn find_refresh_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<RefreshSession>, AuthError> {
        let row = sqlx::query_as::<_, RefreshSessionRow>(
            r#"
            SELECT id, user_id, token_hash, expires_at, revoked_at
            FROM refresh_tokens
            WHERE id = $1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_repo_error)?;

        Ok(row.map(Into::into))
    }

    async fn revoke_refresh_session(&self, session_id: Uuid) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            UPDATE refresh_tokens
            SET revoked_at = NOW()
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(session_id)
        .execute(&self.pool)
        .await
        .map_err(map_repo_error)?;

        Ok(())
    }

    async fn store_email_verification_token(
        &self,
        token: EmailVerificationToken,
    ) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            INSERT INTO email_verification_tokens (id, user_id, token_hash, expires_at, consumed_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(token.id)
        .bind(token.user_id)
        .bind(token.token_hash)
        .bind(token.expires_at)
        .bind(token.consumed_at)
        .execute(&self.pool)
        .await
        .map_err(map_repo_error)?;

        Ok(())
    }

    async fn find_email_verification_token(
        &self,
        token_id: Uuid,
    ) -> Result<Option<EmailVerificationToken>, AuthError> {
        let row = sqlx::query_as::<_, EmailVerificationTokenRow>(
            r#"
            SELECT id, user_id, token_hash, expires_at, consumed_at
            FROM email_verification_tokens
            WHERE id = $1
            "#,
        )
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_repo_error)?;

        Ok(row.map(Into::into))
    }

    async fn consume_email_verification_token(&self, token_id: Uuid) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            UPDATE email_verification_tokens
            SET consumed_at = NOW()
            WHERE id = $1 AND consumed_at IS NULL
            "#,
        )
        .bind(token_id)
        .execute(&self.pool)
        .await
        .map_err(map_repo_error)?;

        Ok(())
    }

    async fn mark_email_verified(&self, user_id: Uuid) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            UPDATE users
            SET email_verified = TRUE
            WHERE id = $1
            "#,
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map_err(map_repo_error)?;

        Ok(())
    }
}

impl TryFrom<UserRow> for User {
    type Error = AuthError;

    fn try_from(value: UserRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: value.id,
            email: value.email,
            password_hash: value.password_hash,
            role: value
                .role
                .parse::<UserRole>()
                .map_err(|err| AuthError::Repository(err.to_owned()))?,
            email_verified: value.email_verified,
            mfa_enabled: value.mfa_enabled,
            status: value.status,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl From<RefreshSessionRow> for RefreshSession {
    fn from(value: RefreshSessionRow) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            token_hash: value.token_hash,
            expires_at: value.expires_at,
            revoked_at: value.revoked_at,
        }
    }
}

impl From<EmailVerificationTokenRow> for EmailVerificationToken {
    fn from(value: EmailVerificationTokenRow) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            token_hash: value.token_hash,
            expires_at: value.expires_at,
            consumed_at: value.consumed_at,
        }
    }
}

fn map_repo_error(err: sqlx::Error) -> AuthError {
    match &err {
        sqlx::Error::Database(db_err)
            if db_err.code().as_deref() == Some("23505") && db_err.message().contains("users") =>
        {
            AuthError::EmailAlreadyExists
        }
        _ => AuthError::Repository(err.to_string()),
    }
}
