use std::sync::Arc;

use api::{
    config::{Config, DatabaseConfig, Environment, JwtConfig, ObservabilityConfig, ServerConfig},
    metrics::MetricsRegistry,
    router::build_router,
    state::AppState,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use domain::auth::{User, UserRole};
use infrastructure::{
    auth::{
        hasher::Argon2PasswordHasher,
        jwt::{JwtConfig as InfraJwtConfig, JwtTokenService},
        repo::PgAuthRepository,
    },
    notifications::NoopEmailSender,
    persistence::repos::{
        PgAuditLogRepository, PgChildProfileRepository, PgContributionRepository,
        PgContributionWorkflowRepository, PgAchievementCredentialRepository, PgMilestoneRepository,
        PgNotificationPreferenceRepository, PgNotificationRepository, PgPayoutRepository,
        PgPayoutWorkflowRepository, PgScholarshipApplicationRepository,
        PgScholarshipAwardRepository, PgScholarshipPoolRepository, PgScholarshipWorkflowRepository,
        PgSchoolRepository, PgSchoolWorkflowRepository, PgVaultRepository,
    },
    testing::TestDatabase,
};
use serde_json::Value;
use sqlx::PgPool;
use tower::util::ServiceExt;
use uuid::Uuid;

pub struct TestApp {
    pub _db: TestDatabase,
    pub router: Router,
    pub pool: PgPool,
    pub token_service: JwtTokenService,
}

impl TestApp {
    pub async fn new() -> Option<Self> {
        let db = TestDatabase::isolated().await.expect("db setup should not error")?;
        let pool = db.pool().clone();
        let auth_repo = PgAuthRepository::new(pool.clone());
        let child_profile_repo = PgChildProfileRepository::new(pool.clone());
        let vault_repo = PgVaultRepository::new(pool.clone());
        let contribution_repo = PgContributionRepository::new(pool.clone());
        let contribution_workflow_repo = PgContributionWorkflowRepository::new(pool.clone());
        let milestone_repo = PgMilestoneRepository::new(pool.clone());
        let school_repo = PgSchoolRepository::new(pool.clone());
        let school_workflow_repo = PgSchoolWorkflowRepository::new(pool.clone());
        let payout_repo = PgPayoutRepository::new(pool.clone());
        let payout_workflow_repo = PgPayoutWorkflowRepository::new(pool.clone());
        let scholarship_pool_repo = PgScholarshipPoolRepository::new(pool.clone());
        let scholarship_application_repo = PgScholarshipApplicationRepository::new(pool.clone());
        let scholarship_award_repo = PgScholarshipAwardRepository::new(pool.clone());
        let scholarship_workflow_repo = PgScholarshipWorkflowRepository::new(pool.clone());
        let audit_repo = PgAuditLogRepository::new(pool.clone());
        let achievement_credential_repo = PgAchievementCredentialRepository::new(pool.clone());
        let notification_repo = PgNotificationRepository::new(pool.clone());
        let notification_preference_repo = PgNotificationPreferenceRepository::new(pool.clone());
        let email_sender = NoopEmailSender;
        let password_hasher = Argon2PasswordHasher;
        let token_service = JwtTokenService::new(InfraJwtConfig {
            access_secret: "access-secret-access-secret-1234".into(),
            refresh_secret: "refresh-secret-refresh-secret-1234".into(),
            access_ttl_secs: 900,
            refresh_ttl_secs: 2_592_000,
        });

        let state = Arc::new(AppState::new(
            Config {
                app_name: "EDUstell".to_owned(),
                environment: Environment::Local,
                server: ServerConfig { host: "127.0.0.1".to_owned(), port: 8080 },
                database: DatabaseConfig {
                    url: infrastructure::testing::configured_test_database_url()
                        .expect("db url should exist once test db is initialized"),
                },
                jwt: JwtConfig {
                    access_secret: "access-secret-access-secret-1234".to_owned(),
                    refresh_secret: "refresh-secret-refresh-secret-1234".to_owned(),
                    access_ttl_secs: 900,
                    refresh_ttl_secs: 2_592_000,
                },
                observability: ObservabilityConfig {
                    rust_log: "info".to_owned(),
                    log_format: "pretty".to_owned(),
                },
            },
            MetricsRegistry::new(),
            pool.clone(),
            auth_repo,
            child_profile_repo,
            vault_repo,
            contribution_repo,
            contribution_workflow_repo,
            milestone_repo,
            school_repo,
            school_workflow_repo,
            payout_repo,
            payout_workflow_repo,
            scholarship_pool_repo,
            scholarship_application_repo,
            scholarship_award_repo,
            scholarship_workflow_repo,
            audit_repo,
            achievement_credential_repo,
            notification_repo,
            notification_preference_repo,
            email_sender,
            password_hasher,
            token_service.clone(),
        ));

        let router = build_router(state);
        Some(Self { _db: db, router, pool, token_service })
    }

    pub async fn json_request(
        &self,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Value,
    ) -> (StatusCode, Value) {
        let (status, _, json) = self.json_request_with_headers(method, uri, token, body).await;
        (status, json)
    }

    pub async fn get_json(
        &self,
        uri: &str,
        token: Option<&str>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder().method("GET").uri(uri);
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }

        let response =
            self.router.clone().oneshot(builder.body(Body::empty()).unwrap()).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or_else(|_| serde_json::json!({}));
        (status, headers, json)
    }

    pub async fn json_request_with_headers(
        &self,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Value,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut builder = Request::builder().method(method).uri(uri);
        builder = builder.header("content-type", "application/json");
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let response = self
            .router
            .clone()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or_else(|_| serde_json::json!({}));
        (status, headers, json)
    }

    pub async fn seed_user(&self, email: &str, role: UserRole) -> User {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO users (id, email, password_hash, role, email_verified, mfa_enabled, status)
            VALUES ($1, $2, 'hashed-password', $3, TRUE, FALSE, 'active')
            "#,
        )
        .bind(id)
        .bind(email)
        .bind(role.as_str())
        .execute(&self.pool)
        .await
        .unwrap();
        let row = sqlx::query_as::<_, (
            Uuid,
            String,
            String,
            bool,
            bool,
            String,
            chrono::DateTime<chrono::Utc>,
            chrono::DateTime<chrono::Utc>,
        )>(
            r#"SELECT id, email, role, email_verified, mfa_enabled, status, created_at, updated_at FROM users WHERE id = $1"#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        User {
            id: row.0,
            email: row.1,
            password_hash: "hashed-password".to_owned(),
            role: row.2.parse().unwrap(),
            email_verified: row.3,
            mfa_enabled: row.4,
            status: row.5,
            created_at: row.6,
            updated_at: row.7,
        }
    }

    pub fn access_token(&self, user: &User) -> String {
        use application::ports::TokenService;
        self.token_service.issue_access_token(user, Uuid::new_v4()).unwrap().token
    }

    pub async fn seed_vault_bundle(&self, owner_user_id: Uuid) -> (Uuid, Uuid, Uuid) {
        let child_id = Uuid::new_v4();
        let plan_id = Uuid::new_v4();
        let vault_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO child_profiles (id, owner_user_id, full_name, education_level)
               VALUES ($1, $2, 'Student', 'secondary')"#,
        )
        .bind(child_id)
        .bind(owner_user_id)
        .execute(&self.pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO savings_plans (id, child_profile_id, owner_user_id, name, target_amount_minor, target_currency, status)
               VALUES ($1, $2, $3, 'Plan', 100000, 'USD', 'draft')"#,
        )
        .bind(plan_id)
        .bind(child_id)
        .bind(owner_user_id)
        .execute(&self.pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO savings_vaults
               (id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, total_disbursed_minor, version)
               VALUES ($1, $2, $3, 'USD', 'active', 0, 0, 0, 0)"#,
        )
        .bind(vault_id)
        .bind(plan_id)
        .bind(owner_user_id)
        .execute(&self.pool)
        .await
        .unwrap();
        (child_id, plan_id, vault_id)
    }

    pub async fn seed_milestone(&self, vault_id: Uuid) -> Uuid {
        let milestone_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO milestones
               (id, vault_id, title, due_date, target_amount_minor, funded_amount_minor, currency, payout_type, status)
               VALUES ($1, $2, 'Term 1', CURRENT_DATE, 5000, 0, 'USD', 'tuition', 'planned')"#,
        )
        .bind(milestone_id)
        .bind(vault_id)
        .execute(&self.pool)
        .await
        .unwrap();
        milestone_id
    }

    pub async fn seed_verified_school(&self, admin_user_id: Uuid) -> Uuid {
        let school_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO schools
               (id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at)
               VALUES ($1, 'Springfield High', 'Springfield High', 'NG', 'manual', 'acct-001', 'verified', $2, NOW())"#,
        )
        .bind(school_id)
        .bind(admin_user_id)
        .execute(&self.pool)
        .await
        .unwrap();
        school_id
    }
}
