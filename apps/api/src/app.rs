use std::{net::SocketAddr, sync::Arc};

use infrastructure::auth::{
    hasher::Argon2PasswordHasher,
    jwt::{JwtConfig as InfraJwtConfig, JwtTokenService},
    repo::PgAuthRepository,
};
use infrastructure::db;
use infrastructure::notifications::NoopEmailSender;
use infrastructure::persistence::repos::{
    PgAchievementCredentialRepository, PgAuditLogRepository, PgChildProfileRepository,
    PgContributionRepository, PgContributionWorkflowRepository, PgMilestoneRepository,
    PgNotificationPreferenceRepository, PgNotificationRepository, PgPayoutRepository,
    PgPayoutWorkflowRepository, PgScholarshipApplicationRepository, PgScholarshipAwardRepository,
    PgScholarshipPoolRepository, PgScholarshipWorkflowRepository, PgSchoolRepository,
    PgSchoolWorkflowRepository, PgVaultRepository,
};
use tokio::net::TcpListener;
use tracing::info;

use crate::{config::Config, error::InternalError, metrics::MetricsRegistry, router, state::AppState};

pub struct Application {
    listener: TcpListener,
    router: axum::Router,
    local_addr: SocketAddr,
}

impl Application {
    pub async fn build(config: Config) -> Result<Self, InternalError> {
        let pool = db::pool::connect(&config.database.url).await?;
        db::migrations::run(&pool).await?;
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
            access_secret: config.jwt.access_secret.clone().into(),
            refresh_secret: config.jwt.refresh_secret.clone().into(),
            access_ttl_secs: config.jwt.access_ttl_secs,
            refresh_ttl_secs: config.jwt.refresh_ttl_secs,
        });

        let listener =
            TcpListener::bind(config.server.socket_addr()).await.map_err(InternalError::Io)?;
        let local_addr = listener.local_addr().map_err(InternalError::Io)?;
        let metrics = MetricsRegistry::new();

        let state = Arc::new(AppState::new(
            config,
            metrics,
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
        let router = router::build_router(state);

        Ok(Self { listener, router, local_addr })
    }

    pub async fn run(self) -> Result<(), InternalError> {
        info!(address = %self.local_addr, "starting api server");

        axum::serve(self.listener, self.router)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(InternalError::Io)
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};

        match signal(SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
