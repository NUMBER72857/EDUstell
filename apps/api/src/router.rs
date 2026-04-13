use std::sync::Arc;

use axum::{
    Router, middleware,
    routing::{get, post, put},
};

use crate::{
    api,
    middleware::{logging, metrics, panic_boundary, rate_limit, request_id, security_headers},
    routes,
    state::AppState,
};

pub fn build_router(state: Arc<AppState>) -> Router {
    let v1 = Router::new()
        .route("/health", get(routes::health::health))
        .route("/health/live", get(routes::health::health))
        .route("/ready", get(routes::health::ready))
        .route("/health/ready", get(routes::health::ready))
        .nest(
            "/auth",
            Router::new()
                .route("/register", post(routes::auth::register))
                .route("/login", post(routes::auth::login))
                .route("/refresh", post(routes::auth::refresh))
                .route("/logout", post(routes::auth::logout))
                .route("/me", get(routes::auth::me))
                .route("/verify-email", post(routes::auth::verify_email))
                .route("/verify-email/begin", post(routes::auth::begin_email_verification))
                .route("/platform-admin/ping", get(routes::auth::platform_admin_ping)),
        )
        .route(
            "/credentials",
            post(routes::credentials::issue).get(routes::credentials::list),
        )
        .route("/credentials/{id}", get(routes::credentials::get))
        .route(
            "/vaults/{id}/contributions",
            post(routes::contributions::create_contribution)
                .get(routes::contributions::list_contributions),
        )
        .route(
            "/vaults/{id}/payout-requests",
            post(routes::payouts::create).get(routes::payouts::list_by_vault),
        )
        .route("/payouts/{id}", get(routes::payouts::get))
        .route("/payouts/{id}/under-review", post(routes::payouts::move_to_review))
        .route("/payouts/{id}/approve", post(routes::payouts::approve))
        .route("/payouts/{id}/reject", post(routes::payouts::reject))
        .route("/payouts/{id}/processing", post(routes::payouts::processing))
        .route("/payouts/{id}/complete", post(routes::payouts::complete))
        .route("/payouts/{id}/fail", post(routes::payouts::fail))
        .route(
            "/scholarship-pools",
            post(routes::scholarships::create_pool).get(routes::scholarships::list_pools),
        )
        .route("/scholarship-pools/{id}", get(routes::scholarships::get_pool))
        .route("/scholarship-pools/{id}/fund", post(routes::scholarships::fund_pool))
        .route(
            "/scholarship-pools/{id}/applications",
            post(routes::scholarships::create_application)
                .get(routes::scholarships::list_applications),
        )
        .route("/scholarship-pools/{id}/awards", get(routes::scholarships::list_awards))
        .route(
            "/scholarship-pools/{id}/contributions",
            get(routes::scholarships::list_donor_contributions),
        )
        .route(
            "/scholarship-applications/{id}/approve",
            post(routes::scholarships::approve_application),
        )
        .route(
            "/scholarship-applications/{id}/reject",
            post(routes::scholarships::reject_application),
        )
        .route("/scholarship-awards/{id}/disburse", post(routes::scholarships::disburse_award))
        .route("/scholarship-awards/{id}/revoke", post(routes::scholarships::revoke_award))
        .route("/contributions/{id}/confirm", post(routes::contributions::confirm))
        .route("/contributions/{id}/fail", post(routes::contributions::fail))
        .route("/contributions/{id}/reverse", post(routes::contributions::reverse))
        .route("/notifications", get(routes::notifications::list))
        .route("/notifications/{id}/read", post(routes::notifications::mark_read))
        .route("/notifications/{id}/unread", post(routes::notifications::mark_unread))
        .route("/notification-preferences", get(routes::notifications::list_preferences))
        .route(
            "/notification-preferences/{notification_type}",
            put(routes::notifications::upsert_preference),
        )
        .route("/schools", post(routes::schools::create).get(routes::schools::search))
        .route("/schools/{id}/verify", post(routes::schools::verify))
        .route("/admin/audit-logs", get(routes::observability::list_audit_logs))
        .route("/internal/metrics", get(routes::observability::metrics));

    Router::new()
        .route("/api/docs", get(|| async { api::docs_response() }))
        .route("/api/openapi.json", get(|| async { api::openapi_response() }))
        .route("/credentials", get(routes::credential_pages::learner_credentials_page))
        .route("/issuer/credentials", get(routes::credential_pages::issuer_credentials_page))
        .nest("/api/v1", v1)
        .layer(panic_boundary::layer())
        .layer(middleware::from_fn(rate_limit::auth_rate_limit))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            security_headers::apply_security_headers,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), metrics::track_requests))
        .layer(middleware::from_fn(logging::log_requests))
        .layer(middleware::from_fn(request_id::attach_request_context))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use application::ports::TokenService;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use domain::auth::{User, UserRole};
    use infrastructure::{
        auth::{
            hasher::Argon2PasswordHasher,
            jwt::{JwtConfig, JwtTokenService},
            repo::PgAuthRepository,
        },
        db::pool::DatabasePool,
        notifications::NoopEmailSender,
        persistence::repos::{
            PgAuditLogRepository, PgChildProfileRepository, PgContributionRepository,
            PgContributionWorkflowRepository, PgAchievementCredentialRepository,
            PgMilestoneRepository,
            PgNotificationPreferenceRepository, PgNotificationRepository, PgPayoutRepository,
            PgPayoutWorkflowRepository, PgScholarshipApplicationRepository,
            PgScholarshipAwardRepository, PgScholarshipPoolRepository,
            PgScholarshipWorkflowRepository, PgSchoolRepository, PgSchoolWorkflowRepository,
            PgVaultRepository,
        },
    };
    use sqlx::postgres::PgPoolOptions;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::{config::Config, metrics::MetricsRegistry, state::AppState};

    use super::build_router;

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let pool: DatabasePool = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/edustell")
            .expect("lazy pool should build");
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
        let token_service = JwtTokenService::new(JwtConfig {
            access_secret: "access-secret-access-secret-1234".into(),
            refresh_secret: "refresh-secret-refresh-secret-1234".into(),
            access_ttl_secs: 900,
            refresh_ttl_secs: 2_592_000,
        });
        let state = Arc::new(AppState::new(
            Config::test(),
            MetricsRegistry::new(),
            pool,
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
            token_service,
        ));

        let response = build_router(state)
            .oneshot(Request::builder().uri("/api/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn platform_admin_route_rejects_missing_token() {
        let state = test_state();

        let response = build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/platform-admin/ping")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_sets_request_and_correlation_ids() {
        let state = test_state();

        let response = build_router(state)
            .oneshot(Request::builder().uri("/api/v1/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert!(response.headers().contains_key("x-request-id"));
        assert!(response.headers().contains_key("x-correlation-id"));
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    }

    #[tokio::test]
    async fn platform_admin_route_rejects_wrong_role() {
        let state = test_state();
        let token = state
            .token_service
            .issue_access_token(&test_user(UserRole::Donor), Uuid::new_v4())
            .unwrap()
            .token;

        let response = build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/platform-admin/ping")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn platform_admin_route_allows_platform_admin() {
        let state = test_state();
        let token = state
            .token_service
            .issue_access_token(&test_user(UserRole::PlatformAdmin), Uuid::new_v4())
            .unwrap()
            .token;

        let response = build_router(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/platform-admin/ping")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    fn test_state() -> Arc<AppState> {
        let pool: DatabasePool = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/edustell")
            .expect("lazy pool should build");
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
        let token_service = JwtTokenService::new(JwtConfig {
            access_secret: "access-secret-access-secret-1234".into(),
            refresh_secret: "refresh-secret-refresh-secret-1234".into(),
            access_ttl_secs: 900,
            refresh_ttl_secs: 2_592_000,
        });

        Arc::new(AppState::new(
            Config::test(),
            MetricsRegistry::new(),
            pool,
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
            token_service,
        ))
    }

    fn test_user(role: UserRole) -> User {
        User {
            id: Uuid::new_v4(),
            email: "user@example.com".to_owned(),
            password_hash: "unused".to_owned(),
            role,
            email_verified: false,
            mfa_enabled: false,
            status: "active".to_owned(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}
