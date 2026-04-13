use application::repos::{
    AchievementCredentialRepository, AuditLogRepository, BlockchainTransactionRepository,
    ChildProfileRepository, ContributionRepository, ContributionSettlementResult,
    ContributionWorkflowRepository, DonorContributionRepository, ExternalReferenceRepository,
    KycRepository, MilestoneRepository, NotificationPreferenceRepository,
    NotificationRepository, PayoutRepository, PayoutTransitionResult, PayoutWorkflowRepository,
    PersistenceError, SavingsPlanRepository, ScholarshipApplicationRepository,
    ScholarshipAwardRepository, ScholarshipPoolRepository, ScholarshipReviewDecisionResult,
    ScholarshipWorkflowRepository, SchoolRepository, SchoolWorkflowRepository,
    VaultLedgerRepository, VaultRepository, WalletAccountRepository,
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use domain::{
    contributions::{VaultLedgerEntry, VaultLedgerEntryType},
    persistence::{
        AchievementCredential, AchievementCredentialStatus, AchievementCredentialType, AuditLog,
        BlockchainTransactionRecord, BlockchainTransactionStatus, ChildProfile, Contribution,
        ContributionStatus, DonorContribution, ExternalReference, ExternalReferenceEntityType,
        ExternalReferenceKind, KycProfile, Milestone, Notification, NotificationPreference,
        PayoutRequest, SavingsPlan, SavingsVault, ScholarshipApplication,
        ScholarshipApplicationStatus, ScholarshipAward, ScholarshipAwardStatus, ScholarshipPool,
        ScholarshipPoolStatus, School, VaultContributor, WalletAccount,
    },
};
use serde_json::Value;
use shared::{currency::Currency, money::Money};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

macro_rules! pg_repo {
    ($name:ident) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            pool: PgPool,
        }

        impl $name {
            pub fn new(pool: PgPool) -> Self {
                Self { pool }
            }
        }
    };
}

pg_repo!(PgChildProfileRepository);
pg_repo!(PgSavingsPlanRepository);
pg_repo!(PgVaultRepository);
pg_repo!(PgMilestoneRepository);
pg_repo!(PgContributionRepository);
pg_repo!(PgContributionWorkflowRepository);
pg_repo!(PgSchoolRepository);
pg_repo!(PgSchoolWorkflowRepository);
pg_repo!(PgPayoutRepository);
pg_repo!(PgPayoutWorkflowRepository);
pg_repo!(PgKycRepository);
pg_repo!(PgScholarshipPoolRepository);
pg_repo!(PgScholarshipApplicationRepository);
pg_repo!(PgScholarshipAwardRepository);
pg_repo!(PgAchievementCredentialRepository);
pg_repo!(PgScholarshipWorkflowRepository);
pg_repo!(PgAuditLogRepository);
pg_repo!(PgNotificationRepository);
pg_repo!(PgNotificationPreferenceRepository);
pg_repo!(PgWalletAccountRepository);
pg_repo!(PgExternalReferenceRepository);
pg_repo!(PgBlockchainTransactionRepository);

#[derive(Debug, Clone, Default)]
pub struct AuditLogQuery {
    pub actor_user_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub action: Option<String>,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub limit: Option<i64>,
}

impl PgAuditLogRepository {
    pub async fn query(&self, filter: AuditLogQuery) -> Result<Vec<AuditLog>, PersistenceError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, actor_user_id, entity_type, entity_id, action, request_id, correlation_id, metadata, created_at, updated_at FROM audit_logs",
        );

        let mut has_where = false;
        if let Some(actor_user_id) = filter.actor_user_id {
            push_where(&mut builder, &mut has_where, "actor_user_id");
            builder.push_bind(actor_user_id);
        }
        if let Some(entity_type) = filter.entity_type {
            push_where(&mut builder, &mut has_where, "entity_type");
            builder.push_bind(entity_type);
        }
        if let Some(entity_id) = filter.entity_id {
            push_where(&mut builder, &mut has_where, "entity_id");
            builder.push_bind(entity_id);
        }
        if let Some(action) = filter.action {
            push_where(&mut builder, &mut has_where, "action");
            builder.push_bind(action);
        }
        if let Some(request_id) = filter.request_id {
            push_where(&mut builder, &mut has_where, "request_id");
            builder.push_bind(request_id);
        }
        if let Some(correlation_id) = filter.correlation_id {
            push_where(&mut builder, &mut has_where, "correlation_id");
            builder.push_bind(correlation_id);
        }

        builder.push(" ORDER BY created_at DESC");
        builder.push(" LIMIT ");
        builder.push_bind(filter.limit.unwrap_or(100).clamp(1, 500));

        builder
            .build_query_as::<AuditLogRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_err)
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }
}

#[derive(Debug, FromRow)]
struct ChildProfileRow {
    id: Uuid,
    owner_user_id: Uuid,
    full_name: String,
    date_of_birth: Option<NaiveDate>,
    education_level: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct SavingsPlanRow {
    id: Uuid,
    child_profile_id: Uuid,
    owner_user_id: Uuid,
    name: String,
    description: Option<String>,
    target_amount_minor: i64,
    target_currency: String,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct SavingsVaultRow {
    id: Uuid,
    plan_id: Uuid,
    owner_user_id: Uuid,
    currency: String,
    status: String,
    total_contributed_minor: i64,
    total_locked_minor: i64,
    total_disbursed_minor: i64,
    external_wallet_account_id: Option<Uuid>,
    external_contract_ref: Option<String>,
    version: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct VaultContributorRow {
    id: Uuid,
    vault_id: Uuid,
    contributor_user_id: Uuid,
    role_label: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct MilestoneRow {
    id: Uuid,
    vault_id: Uuid,
    title: String,
    description: Option<String>,
    due_date: NaiveDate,
    target_amount_minor: i64,
    funded_amount_minor: i64,
    currency: String,
    payout_type: String,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ContributionRow {
    id: Uuid,
    vault_id: Uuid,
    contributor_user_id: Uuid,
    amount_minor: i64,
    currency: String,
    status: String,
    source_type: String,
    external_reference: Option<String>,
    idempotency_key: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct VaultLedgerEntryRow {
    id: Uuid,
    vault_id: Uuid,
    contribution_id: Option<Uuid>,
    actor_user_id: Option<Uuid>,
    entry_type: String,
    amount_minor: i64,
    currency: String,
    balance_after_minor: i64,
    external_reference: Option<String>,
    metadata: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct SchoolRow {
    id: Uuid,
    legal_name: String,
    display_name: String,
    country: String,
    payout_method: String,
    payout_reference: String,
    verification_status: String,
    verified_by: Option<Uuid>,
    verified_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct PayoutRequestRow {
    id: Uuid,
    vault_id: Uuid,
    milestone_id: Uuid,
    school_id: Uuid,
    requested_by: Uuid,
    amount_minor: i64,
    currency: String,
    idempotency_key: Option<String>,
    status: String,
    review_notes: Option<String>,
    external_payout_reference: Option<String>,
    reviewed_by: Option<Uuid>,
    reviewed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct KycProfileRow {
    id: Uuid,
    user_id: Uuid,
    status: String,
    provider_reference: Option<String>,
    reviewed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ScholarshipPoolRow {
    id: Uuid,
    owner_user_id: Uuid,
    name: String,
    description: Option<String>,
    status: String,
    available_funds_minor: i64,
    currency: String,
    geography_restriction: Option<String>,
    education_level_restriction: Option<String>,
    school_id_restriction: Option<Uuid>,
    category_restriction: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ScholarshipApplicationRow {
    id: Uuid,
    pool_id: Uuid,
    applicant_user_id: Uuid,
    child_profile_id: Uuid,
    student_country: Option<String>,
    education_level: Option<String>,
    school_id: Option<Uuid>,
    category: Option<String>,
    status: String,
    notes: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ScholarshipAwardRow {
    id: Uuid,
    application_id: Uuid,
    decided_by: Uuid,
    amount_minor: i64,
    currency: String,
    status: String,
    decision_notes: Option<String>,
    linked_payout_request_id: Option<Uuid>,
    linked_vault_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct AchievementCredentialRow {
    id: Uuid,
    credential_ref: Uuid,
    child_profile_id: Uuid,
    recipient_user_id: Option<Uuid>,
    school_id: Option<Uuid>,
    achievement_type: String,
    status: String,
    title: String,
    description: Option<String>,
    achievement_date: NaiveDate,
    issued_by_user_id: Uuid,
    issued_by_role: String,
    issuance_notes: Option<String>,
    evidence_uri: Option<String>,
    attestation_hash: String,
    attestation_method: String,
    attestation_anchor: Option<String>,
    attestation_anchor_network: Option<String>,
    metadata: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct DonorContributionRow {
    id: Uuid,
    pool_id: Uuid,
    donor_user_id: Uuid,
    amount_minor: i64,
    currency: String,
    status: String,
    external_reference: Option<String>,
    idempotency_key: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct AuditLogRow {
    id: Uuid,
    actor_user_id: Option<Uuid>,
    entity_type: String,
    entity_id: Option<Uuid>,
    action: String,
    request_id: Option<String>,
    correlation_id: Option<String>,
    metadata: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct NotificationRow {
    id: Uuid,
    user_id: Uuid,
    notification_type: String,
    title: String,
    body: String,
    metadata: Value,
    status: String,
    read_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct NotificationPreferenceRow {
    id: Uuid,
    user_id: Uuid,
    notification_type: String,
    in_app_enabled: bool,
    email_enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct WalletAccountRow {
    id: Uuid,
    user_id: Uuid,
    network: String,
    address: String,
    label: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ExternalReferenceRow {
    id: Uuid,
    entity_type: String,
    entity_id: Uuid,
    reference_kind: String,
    value: String,
    metadata: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct BlockchainTransactionRecordRow {
    id: Uuid,
    entity_type: String,
    entity_id: Uuid,
    operation_kind: String,
    idempotency_key: String,
    status: String,
    tx_hash: Option<String>,
    attempt_count: i32,
    last_error_code: Option<String>,
    last_error_message: Option<String>,
    next_retry_at: Option<DateTime<Utc>>,
    metadata: Value,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[async_trait]
impl ChildProfileRepository for PgChildProfileRepository {
    async fn create(&self, profile: ChildProfile) -> Result<ChildProfile, PersistenceError> {
        sqlx::query_as::<_, ChildProfileRow>(
            r#"
            INSERT INTO child_profiles (id, owner_user_id, full_name, date_of_birth, education_level)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, owner_user_id, full_name, date_of_birth, education_level, created_at, updated_at
            "#,
        )
        .bind(profile.id)
        .bind(profile.owner_user_id)
        .bind(profile.full_name)
        .bind(profile.date_of_birth)
        .bind(profile.education_level)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ChildProfile>, PersistenceError> {
        sqlx::query_as::<_, ChildProfileRow>(
            r#"SELECT id, owner_user_id, full_name, date_of_birth, education_level, created_at, updated_at FROM child_profiles WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn list_by_owner(
        &self,
        owner_user_id: Uuid,
    ) -> Result<Vec<ChildProfile>, PersistenceError> {
        sqlx::query_as::<_, ChildProfileRow>(
            r#"SELECT id, owner_user_id, full_name, date_of_birth, education_level, created_at, updated_at
               FROM child_profiles WHERE owner_user_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(owner_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }
}

#[async_trait]
impl SavingsPlanRepository for PgSavingsPlanRepository {
    async fn create(&self, plan: SavingsPlan) -> Result<SavingsPlan, PersistenceError> {
        sqlx::query_as::<_, SavingsPlanRow>(
            r#"INSERT INTO savings_plans
               (id, child_profile_id, owner_user_id, name, description, target_amount_minor, target_currency, status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
               RETURNING id, child_profile_id, owner_user_id, name, description, target_amount_minor, target_currency, status, created_at, updated_at"#,
        )
        .bind(plan.id)
        .bind(plan.child_profile_id)
        .bind(plan.owner_user_id)
        .bind(plan.name)
        .bind(plan.description)
        .bind(plan.target_amount.amount_minor)
        .bind(plan.target_amount.currency.to_string())
        .bind(plan.status.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<SavingsPlan>, PersistenceError> {
        sqlx::query_as::<_, SavingsPlanRow>(
            r#"SELECT id, child_profile_id, owner_user_id, name, description, target_amount_minor, target_currency, status, created_at, updated_at
               FROM savings_plans WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn list_by_child_profile(
        &self,
        child_profile_id: Uuid,
    ) -> Result<Vec<SavingsPlan>, PersistenceError> {
        sqlx::query_as::<_, SavingsPlanRow>(
            r#"SELECT id, child_profile_id, owner_user_id, name, description, target_amount_minor, target_currency, status, created_at, updated_at
               FROM savings_plans WHERE child_profile_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(child_profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }
}

#[async_trait]
impl VaultRepository for PgVaultRepository {
    async fn create(&self, vault: SavingsVault) -> Result<SavingsVault, PersistenceError> {
        sqlx::query_as::<_, SavingsVaultRow>(
            r#"INSERT INTO savings_vaults
               (id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, total_disbursed_minor, external_wallet_account_id, external_contract_ref, version)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
               RETURNING id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, total_disbursed_minor, external_wallet_account_id, external_contract_ref, version, created_at, updated_at"#,
        )
        .bind(vault.id)
        .bind(vault.plan_id)
        .bind(vault.owner_user_id)
        .bind(vault.currency)
        .bind(vault.status.as_str())
        .bind(vault.total_contributed_minor)
        .bind(vault.total_locked_minor)
        .bind(vault.total_disbursed_minor)
        .bind(vault.external_wallet_account_id)
        .bind(vault.external_contract_ref)
        .bind(vault.version)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<SavingsVault>, PersistenceError> {
        sqlx::query_as::<_, SavingsVaultRow>(
            r#"SELECT id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, total_disbursed_minor, external_wallet_account_id, external_contract_ref, version, created_at, updated_at
               FROM savings_vaults WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn update_balances(
        &self,
        id: Uuid,
        total_contributed_minor: i64,
        total_locked_minor: i64,
        expected_version: i64,
    ) -> Result<(), PersistenceError> {
        let result = sqlx::query(
            r#"UPDATE savings_vaults
               SET total_contributed_minor = $2,
                   total_locked_minor = $3,
                   version = version + 1
               WHERE id = $1 AND version = $4"#,
        )
        .bind(id)
        .bind(total_contributed_minor)
        .bind(total_locked_minor)
        .bind(expected_version)
        .execute(&self.pool)
        .await
        .map_err(map_err)?;

        if result.rows_affected() == 0 {
            return Err(PersistenceError::Conflict("vault version mismatch".to_owned()));
        }

        Ok(())
    }

    async fn add_contributor(
        &self,
        contributor: VaultContributor,
    ) -> Result<VaultContributor, PersistenceError> {
        sqlx::query_as::<_, VaultContributorRow>(
            r#"INSERT INTO vault_contributors (id, vault_id, contributor_user_id, role_label)
               VALUES ($1,$2,$3,$4)
               RETURNING id, vault_id, contributor_user_id, role_label, created_at, updated_at"#,
        )
        .bind(contributor.id)
        .bind(contributor.vault_id)
        .bind(contributor.contributor_user_id)
        .bind(contributor.role_label)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .map(Into::into)
    }

    async fn list_contributors(
        &self,
        vault_id: Uuid,
    ) -> Result<Vec<VaultContributor>, PersistenceError> {
        sqlx::query_as::<_, VaultContributorRow>(
            r#"SELECT id, vault_id, contributor_user_id, role_label, created_at, updated_at
               FROM vault_contributors WHERE vault_id = $1 ORDER BY created_at ASC"#,
        )
        .bind(vault_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }
}

impl PgVaultRepository {
    pub async fn find_by_id_for_update(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        id: Uuid,
    ) -> Result<Option<SavingsVault>, PersistenceError> {
        let row = sqlx::query_as::<_, SavingsVaultRow>(
            r#"SELECT id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, total_disbursed_minor, external_wallet_account_id, external_contract_ref, version, created_at, updated_at
               FROM savings_vaults WHERE id = $1 FOR UPDATE"#,
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_err)?;

        row.map(TryInto::try_into).transpose()
    }
}

#[async_trait]
impl MilestoneRepository for PgMilestoneRepository {
    async fn create(&self, milestone: Milestone) -> Result<Milestone, PersistenceError> {
        sqlx::query_as::<_, MilestoneRow>(
            r#"INSERT INTO milestones
               (id, vault_id, title, description, due_date, target_amount_minor, funded_amount_minor, currency, payout_type, status)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
               RETURNING id, vault_id, title, description, due_date, target_amount_minor, funded_amount_minor, currency, payout_type, status, created_at, updated_at"#,
        )
        .bind(milestone.id)
        .bind(milestone.vault_id)
        .bind(milestone.title)
        .bind(milestone.description)
        .bind(milestone.due_date)
        .bind(milestone.target_amount.amount_minor)
        .bind(milestone.funded_amount.amount_minor)
        .bind(milestone.target_amount.currency.to_string())
        .bind(milestone.payout_type.as_str())
        .bind(milestone.status.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Milestone>, PersistenceError> {
        sqlx::query_as::<_, MilestoneRow>(
            r#"SELECT id, vault_id, title, description, due_date, target_amount_minor, funded_amount_minor, currency, payout_type, status, created_at, updated_at
               FROM milestones WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<Milestone>, PersistenceError> {
        sqlx::query_as::<_, MilestoneRow>(
            r#"SELECT id, vault_id, title, description, due_date, target_amount_minor, funded_amount_minor, currency, payout_type, status, created_at, updated_at
               FROM milestones WHERE vault_id = $1 ORDER BY due_date ASC"#,
        )
        .bind(vault_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn update_funded_amount(
        &self,
        id: Uuid,
        funded_amount_minor: i64,
        status: &str,
    ) -> Result<(), PersistenceError> {
        sqlx::query(r#"UPDATE milestones SET funded_amount_minor = $2, status = $3 WHERE id = $1"#)
            .bind(id)
            .bind(funded_amount_minor)
            .bind(status)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }
}

#[async_trait]
impl ContributionRepository for PgContributionRepository {
    async fn create(&self, contribution: Contribution) -> Result<Contribution, PersistenceError> {
        sqlx::query_as::<_, ContributionRow>(
            r#"INSERT INTO contributions
               (id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
               RETURNING id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key, created_at, updated_at"#,
        )
        .bind(contribution.id)
        .bind(contribution.vault_id)
        .bind(contribution.contributor_user_id)
        .bind(contribution.amount.amount_minor)
        .bind(contribution.amount.currency.to_string())
        .bind(contribution.status.as_str())
        .bind(contribution.source_type.as_str())
        .bind(contribution.external_reference)
        .bind(contribution.idempotency_key)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Contribution>, PersistenceError> {
        sqlx::query_as::<_, ContributionRow>(
            r#"SELECT id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key, created_at, updated_at
               FROM contributions WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<Contribution>, PersistenceError> {
        sqlx::query_as::<_, ContributionRow>(
            r#"SELECT id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key, created_at, updated_at
               FROM contributions WHERE idempotency_key = $1"#,
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<Contribution>, PersistenceError> {
        sqlx::query_as::<_, ContributionRow>(
            r#"SELECT id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key, created_at, updated_at
               FROM contributions WHERE vault_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(vault_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: &str,
        external_reference: Option<&str>,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            r#"UPDATE contributions SET status = $2, external_reference = COALESCE($3, external_reference) WHERE id = $1"#,
        )
        .bind(id)
        .bind(status)
        .bind(external_reference)
        .execute(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(())
    }
}

#[async_trait]
impl VaultLedgerRepository for PgContributionRepository {
    async fn list_by_vault(
        &self,
        vault_id: Uuid,
    ) -> Result<Vec<VaultLedgerEntry>, PersistenceError> {
        sqlx::query_as::<_, VaultLedgerEntryRow>(
            r#"SELECT id, vault_id, contribution_id, actor_user_id, entry_type, amount_minor, currency, balance_after_minor, external_reference, metadata, created_at, updated_at
               FROM vault_ledger_entries
               WHERE vault_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(vault_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }
}

#[async_trait]
impl ContributionWorkflowRepository for PgContributionWorkflowRepository {
    async fn create_intent_with_audit(
        &self,
        contribution: Contribution,
        audit_log: AuditLog,
    ) -> Result<Contribution, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;

        let contribution = sqlx::query_as::<_, ContributionRow>(
            r#"INSERT INTO contributions
               (id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
               RETURNING id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key, created_at, updated_at"#,
        )
        .bind(contribution.id)
        .bind(contribution.vault_id)
        .bind(contribution.contributor_user_id)
        .bind(contribution.amount.amount_minor)
        .bind(contribution.amount.currency.to_string())
        .bind(contribution.status.as_str())
        .bind(contribution.source_type.as_str())
        .bind(contribution.external_reference)
        .bind(contribution.idempotency_key)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;

        Ok(contribution)
    }

    async fn settle_with_audit(
        &self,
        contribution_id: Uuid,
        status: &str,
        actor_user_id: Uuid,
        external_reference: Option<&str>,
        audit_log: AuditLog,
    ) -> Result<ContributionSettlementResult, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let current = find_contribution_for_update(&mut tx, contribution_id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        let mut contribution: Contribution = current.try_into()?;
        let target_status: ContributionStatus = parse_status(status)?;

        if contribution.status == target_status {
            let vault = find_vault_for_update(&mut tx, contribution.vault_id)
                .await?
                .ok_or(PersistenceError::NotFound)?;
            tx.rollback().await.map_err(map_err)?;
            return Ok(ContributionSettlementResult { contribution, ledger_entry: None, vault });
        }

        let mut vault = find_vault_for_update(&mut tx, contribution.vault_id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        let updated_row = sqlx::query_as::<_, ContributionRow>(
            r#"UPDATE contributions
               SET status = $2, external_reference = COALESCE($3, external_reference), updated_at = NOW()
               WHERE id = $1
               RETURNING id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key, created_at, updated_at"#,
        )
        .bind(contribution.id)
        .bind(target_status.as_str())
        .bind(external_reference)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)?;
        contribution = updated_row.try_into()?;

        let ledger_entry = match target_status {
            ContributionStatus::Confirmed => {
                vault = update_vault_totals(
                    &mut tx,
                    &vault,
                    vault.total_contributed_minor + contribution.amount.amount_minor,
                )
                .await?;
                Some(
                    insert_ledger_entry(
                        &mut tx,
                        &vault,
                        &contribution,
                        actor_user_id,
                        VaultLedgerEntryType::ContributionConfirmed,
                        contribution.external_reference.as_deref(),
                    )
                    .await?,
                )
            }
            ContributionStatus::Reversed => {
                if vault.total_contributed_minor < contribution.amount.amount_minor {
                    return Err(PersistenceError::Conflict(
                        "vault balance would become negative".to_owned(),
                    ));
                }
                vault = update_vault_totals(
                    &mut tx,
                    &vault,
                    vault.total_contributed_minor - contribution.amount.amount_minor,
                )
                .await?;
                Some(
                    insert_ledger_entry(
                        &mut tx,
                        &vault,
                        &contribution,
                        actor_user_id,
                        VaultLedgerEntryType::ContributionReversed,
                        contribution.external_reference.as_deref(),
                    )
                    .await?,
                )
            }
            ContributionStatus::Failed | ContributionStatus::Pending => None,
        };

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;

        Ok(ContributionSettlementResult { contribution, ledger_entry, vault })
    }
}

#[async_trait]
impl SchoolRepository for PgSchoolRepository {
    async fn create(&self, school: School) -> Result<School, PersistenceError> {
        sqlx::query_as::<_, SchoolRow>(
            r#"INSERT INTO schools
               (id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
               RETURNING id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at, created_at, updated_at"#,
        )
        .bind(school.id)
        .bind(school.legal_name)
        .bind(school.display_name)
        .bind(school.country)
        .bind(school.payout_method.as_str())
        .bind(school.payout_reference)
        .bind(school.verification_status.as_str())
        .bind(school.verified_by)
        .bind(school.verified_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<School>, PersistenceError> {
        sqlx::query_as::<_, SchoolRow>(
            r#"SELECT id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at, created_at, updated_at
               FROM schools WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn search_verified(&self, query: Option<&str>) -> Result<Vec<School>, PersistenceError> {
        let pattern = query.map(|value| format!("%{}%", value.trim().to_lowercase()));

        sqlx::query_as::<_, SchoolRow>(
            r#"SELECT id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at, created_at, updated_at
               FROM schools
               WHERE verification_status = 'verified'
                 AND ($1::TEXT IS NULL OR LOWER(display_name) LIKE $1 OR LOWER(legal_name) LIKE $1)
               ORDER BY display_name ASC"#,
        )
        .bind(pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn list_verified(&self) -> Result<Vec<School>, PersistenceError> {
        sqlx::query_as::<_, SchoolRow>(
            r#"SELECT id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at, created_at, updated_at
               FROM schools WHERE verification_status = 'verified' ORDER BY display_name ASC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn update_verification(
        &self,
        id: Uuid,
        verification_status: &str,
        verified_by: Option<Uuid>,
        verified_at: Option<DateTime<Utc>>,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            r#"UPDATE schools
               SET verification_status = $2, verified_by = $3, verified_at = $4, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(verification_status)
        .bind(verified_by)
        .bind(verified_at)
        .execute(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(())
    }
}

#[async_trait]
impl SchoolWorkflowRepository for PgSchoolWorkflowRepository {
    async fn create_with_audit(
        &self,
        school: School,
        audit_log: AuditLog,
    ) -> Result<School, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let school = sqlx::query_as::<_, SchoolRow>(
            r#"INSERT INTO schools
               (id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
               RETURNING id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at, created_at, updated_at"#,
        )
        .bind(school.id)
        .bind(school.legal_name)
        .bind(school.display_name)
        .bind(school.country)
        .bind(school.payout_method.as_str())
        .bind(school.payout_reference)
        .bind(school.verification_status.as_str())
        .bind(school.verified_by)
        .bind(school.verified_at)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;
        Ok(school)
    }

    async fn verify_with_audit(
        &self,
        school_id: Uuid,
        verification_status: &str,
        verified_by: Uuid,
        audit_log: AuditLog,
    ) -> Result<School, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let verified_at = Utc::now();
        let school = sqlx::query_as::<_, SchoolRow>(
            r#"UPDATE schools
               SET verification_status = $2, verified_by = $3, verified_at = $4, updated_at = NOW()
               WHERE id = $1
               RETURNING id, legal_name, display_name, country, payout_method, payout_reference, verification_status, verified_by, verified_at, created_at, updated_at"#,
        )
        .bind(school_id)
        .bind(verification_status)
        .bind(verified_by)
        .bind(verified_at)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;
        Ok(school)
    }
}

#[async_trait]
impl PayoutRepository for PgPayoutRepository {
    async fn create(&self, payout: PayoutRequest) -> Result<PayoutRequest, PersistenceError> {
        sqlx::query_as::<_, PayoutRequestRow>(
            r#"INSERT INTO payout_requests
               (id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
               RETURNING id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at, created_at, updated_at"#,
        )
        .bind(payout.id)
        .bind(payout.vault_id)
        .bind(payout.milestone_id)
        .bind(payout.school_id)
        .bind(payout.requested_by)
        .bind(payout.amount.amount_minor)
        .bind(payout.amount.currency.to_string())
        .bind(payout.idempotency_key)
        .bind(payout.status.as_str())
        .bind(payout.review_notes)
        .bind(payout.external_payout_reference)
        .bind(payout.reviewed_by)
        .bind(payout.reviewed_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<PayoutRequest>, PersistenceError> {
        sqlx::query_as::<_, PayoutRequestRow>(
            r#"SELECT id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at, created_at, updated_at
               FROM payout_requests WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<PayoutRequest>, PersistenceError> {
        sqlx::query_as::<_, PayoutRequestRow>(
            r#"SELECT id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at, created_at, updated_at
               FROM payout_requests WHERE idempotency_key = $1"#,
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<PayoutRequest>, PersistenceError> {
        sqlx::query_as::<_, PayoutRequestRow>(
            r#"SELECT id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at, created_at, updated_at
               FROM payout_requests WHERE vault_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(vault_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: &str,
        review_notes: Option<&str>,
        external_payout_reference: Option<&str>,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            r#"UPDATE payout_requests
               SET status = $2, review_notes = $3, external_payout_reference = COALESCE($4, external_payout_reference), reviewed_at = NOW()
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(status)
        .bind(review_notes)
        .bind(external_payout_reference)
        .execute(&self.pool)
        .await
        .map_err(map_err)?;
        Ok(())
    }
}

impl PgPayoutRepository {
    pub async fn find_by_id_for_update(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        id: Uuid,
    ) -> Result<Option<PayoutRequest>, PersistenceError> {
        let row = sqlx::query_as::<_, PayoutRequestRow>(
            r#"SELECT id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at, created_at, updated_at
               FROM payout_requests WHERE id = $1 FOR UPDATE"#,
        )
        .bind(id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_err)?;

        row.map(TryInto::try_into).transpose()
    }
}

#[async_trait]
impl PayoutWorkflowRepository for PgPayoutWorkflowRepository {
    async fn create_request_with_audit(
        &self,
        payout: PayoutRequest,
        audit_log: AuditLog,
    ) -> Result<PayoutRequest, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let payout = sqlx::query_as::<_, PayoutRequestRow>(
            r#"INSERT INTO payout_requests
               (id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
               RETURNING id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at, created_at, updated_at"#,
        )
        .bind(payout.id)
        .bind(payout.vault_id)
        .bind(payout.milestone_id)
        .bind(payout.school_id)
        .bind(payout.requested_by)
        .bind(payout.amount.amount_minor)
        .bind(payout.amount.currency.to_string())
        .bind(payout.idempotency_key)
        .bind(payout.status.as_str())
        .bind(payout.review_notes)
        .bind(payout.external_payout_reference)
        .bind(payout.reviewed_by)
        .bind(payout.reviewed_at)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;
        Ok(payout)
    }

    async fn transition_with_audit(
        &self,
        payout_id: Uuid,
        status: &str,
        actor_user_id: Uuid,
        review_notes: Option<&str>,
        external_payout_reference: Option<&str>,
        audit_log: AuditLog,
    ) -> Result<PayoutTransitionResult, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let current =
            find_payout_for_update(&mut tx, payout_id).await?.ok_or(PersistenceError::NotFound)?;
        let mut payout: PayoutRequest = current.try_into()?;
        let target_status = parse_status(status)?;

        if payout.status == target_status {
            let vault = find_vault_for_update(&mut tx, payout.vault_id)
                .await?
                .ok_or(PersistenceError::NotFound)?;
            tx.rollback().await.map_err(map_err)?;
            return Ok(PayoutTransitionResult { payout, vault });
        }

        let mut vault = find_vault_for_update(&mut tx, payout.vault_id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        let updated = sqlx::query_as::<_, PayoutRequestRow>(
            r#"UPDATE payout_requests
               SET status = $2,
                   review_notes = COALESCE($3, review_notes),
                   external_payout_reference = COALESCE($4, external_payout_reference),
                   reviewed_by = $5,
                   reviewed_at = NOW(),
                   updated_at = NOW()
               WHERE id = $1
               RETURNING id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at, created_at, updated_at"#,
        )
        .bind(payout.id)
        .bind(target_status.as_str())
        .bind(review_notes)
        .bind(external_payout_reference)
        .bind(actor_user_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)?;
        payout = updated.try_into()?;

        match target_status {
            domain::persistence::PayoutStatus::Approved => {
                vault = update_vault_locked_and_disbursed(
                    &mut tx,
                    &vault,
                    vault.total_locked_minor + payout.amount.amount_minor,
                    vault.total_disbursed_minor,
                )
                .await?;
            }
            domain::persistence::PayoutStatus::Completed => {
                if vault.total_locked_minor < payout.amount.amount_minor {
                    return Err(PersistenceError::Conflict(
                        "vault locked balance underflow".to_owned(),
                    ));
                }
                vault = update_vault_locked_and_disbursed(
                    &mut tx,
                    &vault,
                    vault.total_locked_minor - payout.amount.amount_minor,
                    vault.total_disbursed_minor + payout.amount.amount_minor,
                )
                .await?;
            }
            domain::persistence::PayoutStatus::Failed
            | domain::persistence::PayoutStatus::Rejected => {
                let release_locked =
                    matches!(target_status, domain::persistence::PayoutStatus::Failed)
                        && vault.total_locked_minor >= payout.amount.amount_minor;
                if release_locked {
                    vault = update_vault_locked_and_disbursed(
                        &mut tx,
                        &vault,
                        vault.total_locked_minor - payout.amount.amount_minor,
                        vault.total_disbursed_minor,
                    )
                    .await?;
                }
            }
            domain::persistence::PayoutStatus::Pending
            | domain::persistence::PayoutStatus::UnderReview
            | domain::persistence::PayoutStatus::Processing => {}
        }

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;
        Ok(PayoutTransitionResult { payout, vault })
    }
}

#[async_trait]
impl KycRepository for PgKycRepository {
    async fn upsert(&self, profile: KycProfile) -> Result<KycProfile, PersistenceError> {
        sqlx::query_as::<_, KycProfileRow>(
            r#"INSERT INTO kyc_profiles (id, user_id, status, provider_reference, reviewed_at)
               VALUES ($1,$2,$3,$4,$5)
               ON CONFLICT (user_id)
               DO UPDATE SET status = EXCLUDED.status, provider_reference = EXCLUDED.provider_reference, reviewed_at = EXCLUDED.reviewed_at
               RETURNING id, user_id, status, provider_reference, reviewed_at, created_at, updated_at"#,
        )
        .bind(profile.id)
        .bind(profile.user_id)
        .bind(profile.status.as_str())
        .bind(profile.provider_reference)
        .bind(profile.reviewed_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_user(&self, user_id: Uuid) -> Result<Option<KycProfile>, PersistenceError> {
        sqlx::query_as::<_, KycProfileRow>(
            r#"SELECT id, user_id, status, provider_reference, reviewed_at, created_at, updated_at FROM kyc_profiles WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }
}

#[async_trait]
impl ScholarshipPoolRepository for PgScholarshipPoolRepository {
    async fn create(&self, pool: ScholarshipPool) -> Result<ScholarshipPool, PersistenceError> {
        sqlx::query_as::<_, ScholarshipPoolRow>(
            r#"INSERT INTO scholarship_pools
               (id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
               RETURNING id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction, created_at, updated_at"#,
        )
        .bind(pool.id)
        .bind(pool.owner_user_id)
        .bind(pool.name)
        .bind(pool.description)
        .bind(pool.status.as_str())
        .bind(pool.available_funds.amount_minor)
        .bind(pool.available_funds.currency.to_string())
        .bind(pool.geography_restriction)
        .bind(pool.education_level_restriction)
        .bind(pool.school_id_restriction)
        .bind(pool.category_restriction)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScholarshipPool>, PersistenceError> {
        sqlx::query_as::<_, ScholarshipPoolRow>(
            r#"SELECT id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction, created_at, updated_at
               FROM scholarship_pools WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn list(&self) -> Result<Vec<ScholarshipPool>, PersistenceError> {
        sqlx::query_as::<_, ScholarshipPoolRow>(
            r#"SELECT id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction, created_at, updated_at
               FROM scholarship_pools ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }
}

#[async_trait]
impl ScholarshipApplicationRepository for PgScholarshipApplicationRepository {
    async fn create(
        &self,
        application: ScholarshipApplication,
    ) -> Result<ScholarshipApplication, PersistenceError> {
        sqlx::query_as::<_, ScholarshipApplicationRow>(
            r#"INSERT INTO scholarship_applications
               (id, pool_id, applicant_user_id, child_profile_id, student_country, education_level, school_id, category, status, notes)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
               RETURNING id, pool_id, applicant_user_id, child_profile_id, student_country, education_level, school_id, category, status, notes, created_at, updated_at"#,
        )
        .bind(application.id)
        .bind(application.pool_id)
        .bind(application.applicant_user_id)
        .bind(application.child_profile_id)
        .bind(application.student_country)
        .bind(application.education_level)
        .bind(application.school_id)
        .bind(application.category)
        .bind(application.status.as_str())
        .bind(application.notes)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn list_by_pool(
        &self,
        pool_id: Uuid,
    ) -> Result<Vec<ScholarshipApplication>, PersistenceError> {
        sqlx::query_as::<_, ScholarshipApplicationRow>(
            r#"SELECT id, pool_id, applicant_user_id, child_profile_id, student_country, education_level, school_id, category, status, notes, created_at, updated_at
               FROM scholarship_applications WHERE pool_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(pool_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ScholarshipApplication>, PersistenceError> {
        sqlx::query_as::<_, ScholarshipApplicationRow>(
            r#"SELECT id, pool_id, applicant_user_id, child_profile_id, student_country, education_level, school_id, category, status, notes, created_at, updated_at
               FROM scholarship_applications WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }
}

#[async_trait]
impl ScholarshipAwardRepository for PgScholarshipAwardRepository {
    async fn create(&self, award: ScholarshipAward) -> Result<ScholarshipAward, PersistenceError> {
        sqlx::query_as::<_, ScholarshipAwardRow>(
            r#"INSERT INTO scholarship_awards (id, application_id, decided_by, amount_minor, currency, status, decision_notes, linked_payout_request_id, linked_vault_id)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
               RETURNING id, application_id, decided_by, amount_minor, currency, status, decision_notes, linked_payout_request_id, linked_vault_id, created_at, updated_at"#,
        )
        .bind(award.id)
        .bind(award.application_id)
        .bind(award.decided_by)
        .bind(award.amount.amount_minor)
        .bind(award.amount.currency.to_string())
        .bind(award.status.as_str())
        .bind(award.decision_notes)
        .bind(award.linked_payout_request_id)
        .bind(award.linked_vault_id)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn list_by_pool(&self, pool_id: Uuid) -> Result<Vec<ScholarshipAward>, PersistenceError> {
        sqlx::query_as::<_, ScholarshipAwardRow>(
            r#"SELECT sa.id, sa.application_id, sa.decided_by, sa.amount_minor, sa.currency, sa.status, sa.decision_notes, sa.linked_payout_request_id, sa.linked_vault_id, sa.created_at, sa.updated_at
               FROM scholarship_awards sa
               INNER JOIN scholarship_applications app ON app.id = sa.application_id
               WHERE app.pool_id = $1
               ORDER BY sa.created_at DESC"#,
        )
        .bind(pool_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScholarshipAward>, PersistenceError> {
        sqlx::query_as::<_, ScholarshipAwardRow>(
            r#"SELECT id, application_id, decided_by, amount_minor, currency, status, decision_notes, linked_payout_request_id, linked_vault_id, created_at, updated_at
               FROM scholarship_awards WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }
}

#[async_trait]
impl AchievementCredentialRepository for PgAchievementCredentialRepository {
    async fn create_with_audit(
        &self,
        credential: AchievementCredential,
        audit_log: AuditLog,
    ) -> Result<AchievementCredential, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let credential = sqlx::query_as::<_, AchievementCredentialRow>(
            r#"INSERT INTO achievement_credentials
               (id, credential_ref, child_profile_id, recipient_user_id, school_id, achievement_type, status, title, description, achievement_date, issued_by_user_id, issued_by_role, issuance_notes, evidence_uri, attestation_hash, attestation_method, attestation_anchor, attestation_anchor_network, metadata)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19)
               RETURNING id, credential_ref, child_profile_id, recipient_user_id, school_id, achievement_type, status, title, description, achievement_date, issued_by_user_id, issued_by_role, issuance_notes, evidence_uri, attestation_hash, attestation_method, attestation_anchor, attestation_anchor_network, metadata, created_at, updated_at"#,
        )
        .bind(credential.id)
        .bind(credential.credential_ref)
        .bind(credential.child_profile_id)
        .bind(credential.recipient_user_id)
        .bind(credential.school_id)
        .bind(credential.achievement_type.as_str())
        .bind(credential.status.as_str())
        .bind(credential.title)
        .bind(credential.description)
        .bind(credential.achievement_date)
        .bind(credential.issued_by_user_id)
        .bind(credential.issued_by_role)
        .bind(credential.issuance_notes)
        .bind(credential.evidence_uri)
        .bind(credential.attestation_hash)
        .bind(credential.attestation_method)
        .bind(credential.attestation_anchor)
        .bind(credential.attestation_anchor_network)
        .bind(credential.metadata)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;

        Ok(credential)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<AchievementCredential>, PersistenceError> {
        sqlx::query_as::<_, AchievementCredentialRow>(
            r#"SELECT id, credential_ref, child_profile_id, recipient_user_id, school_id, achievement_type, status, title, description, achievement_date, issued_by_user_id, issued_by_role, issuance_notes, evidence_uri, attestation_hash, attestation_method, attestation_anchor, attestation_anchor_network, metadata, created_at, updated_at
               FROM achievement_credentials WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn list_by_child_profile(
        &self,
        child_profile_id: Uuid,
    ) -> Result<Vec<AchievementCredential>, PersistenceError> {
        sqlx::query_as::<_, AchievementCredentialRow>(
            r#"SELECT id, credential_ref, child_profile_id, recipient_user_id, school_id, achievement_type, status, title, description, achievement_date, issued_by_user_id, issued_by_role, issuance_notes, evidence_uri, attestation_hash, attestation_method, attestation_anchor, attestation_anchor_network, metadata, created_at, updated_at
               FROM achievement_credentials
               WHERE child_profile_id = $1
               ORDER BY achievement_date DESC, created_at DESC"#,
        )
        .bind(child_profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn list_by_recipient(
        &self,
        recipient_user_id: Uuid,
    ) -> Result<Vec<AchievementCredential>, PersistenceError> {
        sqlx::query_as::<_, AchievementCredentialRow>(
            r#"SELECT id, credential_ref, child_profile_id, recipient_user_id, school_id, achievement_type, status, title, description, achievement_date, issued_by_user_id, issued_by_role, issuance_notes, evidence_uri, attestation_hash, attestation_method, attestation_anchor, attestation_anchor_network, metadata, created_at, updated_at
               FROM achievement_credentials
               WHERE recipient_user_id = $1
               ORDER BY achievement_date DESC, created_at DESC"#,
        )
        .bind(recipient_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn list_by_issuer(
        &self,
        issued_by_user_id: Uuid,
    ) -> Result<Vec<AchievementCredential>, PersistenceError> {
        sqlx::query_as::<_, AchievementCredentialRow>(
            r#"SELECT id, credential_ref, child_profile_id, recipient_user_id, school_id, achievement_type, status, title, description, achievement_date, issued_by_user_id, issued_by_role, issuance_notes, evidence_uri, attestation_hash, attestation_method, attestation_anchor, attestation_anchor_network, metadata, created_at, updated_at
               FROM achievement_credentials
               WHERE issued_by_user_id = $1
               ORDER BY created_at DESC"#,
        )
        .bind(issued_by_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn list_recent(
        &self,
        limit: i64,
    ) -> Result<Vec<AchievementCredential>, PersistenceError> {
        sqlx::query_as::<_, AchievementCredentialRow>(
            r#"SELECT id, credential_ref, child_profile_id, recipient_user_id, school_id, achievement_type, status, title, description, achievement_date, issued_by_user_id, issued_by_role, issuance_notes, evidence_uri, attestation_hash, attestation_method, attestation_anchor, attestation_anchor_network, metadata, created_at, updated_at
               FROM achievement_credentials
               ORDER BY created_at DESC
               LIMIT $1"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }
}

#[async_trait]
impl DonorContributionRepository for PgScholarshipAwardRepository {
    async fn create(
        &self,
        contribution: DonorContribution,
    ) -> Result<DonorContribution, PersistenceError> {
        sqlx::query_as::<_, DonorContributionRow>(
            r#"INSERT INTO donor_contributions
               (id, pool_id, donor_user_id, amount_minor, currency, status, external_reference, idempotency_key)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
               RETURNING id, pool_id, donor_user_id, amount_minor, currency, status, external_reference, idempotency_key, created_at, updated_at"#,
        )
        .bind(contribution.id)
        .bind(contribution.pool_id)
        .bind(contribution.donor_user_id)
        .bind(contribution.amount.amount_minor)
        .bind(contribution.amount.currency.to_string())
        .bind(contribution.status.as_str())
        .bind(contribution.external_reference)
        .bind(contribution.idempotency_key)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn list_by_pool(
        &self,
        pool_id: Uuid,
    ) -> Result<Vec<DonorContribution>, PersistenceError> {
        sqlx::query_as::<_, DonorContributionRow>(
            r#"SELECT id, pool_id, donor_user_id, amount_minor, currency, status, external_reference, idempotency_key, created_at, updated_at
               FROM donor_contributions WHERE pool_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(pool_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }
}

#[async_trait]
impl ScholarshipWorkflowRepository for PgScholarshipWorkflowRepository {
    async fn create_pool_with_audit(
        &self,
        pool: ScholarshipPool,
        audit_log: AuditLog,
    ) -> Result<ScholarshipPool, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let pool = sqlx::query_as::<_, ScholarshipPoolRow>(
            r#"INSERT INTO scholarship_pools
               (id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
               RETURNING id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction, created_at, updated_at"#,
        )
        .bind(pool.id)
        .bind(pool.owner_user_id)
        .bind(pool.name)
        .bind(pool.description)
        .bind(pool.status.as_str())
        .bind(pool.available_funds.amount_minor)
        .bind(pool.available_funds.currency.to_string())
        .bind(pool.geography_restriction)
        .bind(pool.education_level_restriction)
        .bind(pool.school_id_restriction)
        .bind(pool.category_restriction)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;
        Ok(pool)
    }

    async fn fund_pool_with_audit(
        &self,
        contribution: DonorContribution,
        audit_log: AuditLog,
    ) -> Result<(ScholarshipPool, DonorContribution), PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let pool = find_scholarship_pool_for_update(&mut tx, contribution.pool_id)
            .await?
            .ok_or(PersistenceError::NotFound)?;
        let updated_pool = sqlx::query_as::<_, ScholarshipPoolRow>(
            r#"UPDATE scholarship_pools
               SET available_funds_minor = $2, updated_at = NOW()
               WHERE id = $1
               RETURNING id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction, created_at, updated_at"#,
        )
        .bind(pool.id)
        .bind(pool.available_funds.amount_minor + contribution.amount.amount_minor)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        let contribution = sqlx::query_as::<_, DonorContributionRow>(
            r#"INSERT INTO donor_contributions
               (id, pool_id, donor_user_id, amount_minor, currency, status, external_reference, idempotency_key)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
               RETURNING id, pool_id, donor_user_id, amount_minor, currency, status, external_reference, idempotency_key, created_at, updated_at"#,
        )
        .bind(contribution.id)
        .bind(contribution.pool_id)
        .bind(contribution.donor_user_id)
        .bind(contribution.amount.amount_minor)
        .bind(contribution.amount.currency.to_string())
        .bind(contribution.status.as_str())
        .bind(contribution.external_reference)
        .bind(contribution.idempotency_key)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;
        Ok((updated_pool, contribution))
    }

    async fn create_application_with_audit(
        &self,
        application: ScholarshipApplication,
        audit_log: AuditLog,
    ) -> Result<ScholarshipApplication, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let application: ScholarshipApplication = sqlx::query_as::<_, ScholarshipApplicationRow>(
            r#"INSERT INTO scholarship_applications
               (id, pool_id, applicant_user_id, child_profile_id, student_country, education_level, school_id, category, status, notes)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
               RETURNING id, pool_id, applicant_user_id, child_profile_id, student_country, education_level, school_id, category, status, notes, created_at, updated_at"#,
        )
        .bind(application.id)
        .bind(application.pool_id)
        .bind(application.applicant_user_id)
        .bind(application.child_profile_id)
        .bind(application.student_country)
        .bind(application.education_level)
        .bind(application.school_id)
        .bind(application.category)
        .bind(application.status.as_str())
        .bind(application.notes)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;
        Ok(application)
    }

    async fn decide_application_with_audit(
        &self,
        application_id: Uuid,
        application_status: &str,
        award: ScholarshipAward,
        pool_available_funds_minor: i64,
        audit_log: AuditLog,
    ) -> Result<ScholarshipReviewDecisionResult, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let application: ScholarshipApplication = sqlx::query_as::<_, ScholarshipApplicationRow>(
            r#"UPDATE scholarship_applications
               SET status = $2, updated_at = NOW()
               WHERE id = $1
               RETURNING id, pool_id, applicant_user_id, child_profile_id, student_country, education_level, school_id, category, status, notes, created_at, updated_at"#,
        )
        .bind(application_id)
        .bind(application_status)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        let pool = sqlx::query_as::<_, ScholarshipPoolRow>(
            r#"UPDATE scholarship_pools
               SET available_funds_minor = $2, updated_at = NOW()
               WHERE id = $1
               RETURNING id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction, created_at, updated_at"#,
        )
        .bind(application.pool_id)
        .bind(pool_available_funds_minor)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        let award: ScholarshipAward = sqlx::query_as::<_, ScholarshipAwardRow>(
            r#"INSERT INTO scholarship_awards
               (id, application_id, decided_by, amount_minor, currency, status, decision_notes, linked_payout_request_id, linked_vault_id)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
               RETURNING id, application_id, decided_by, amount_minor, currency, status, decision_notes, linked_payout_request_id, linked_vault_id, created_at, updated_at"#,
        )
        .bind(award.id)
        .bind(award.application_id)
        .bind(award.decided_by)
        .bind(award.amount.amount_minor)
        .bind(award.amount.currency.to_string())
        .bind(award.status.as_str())
        .bind(award.decision_notes)
        .bind(award.linked_payout_request_id)
        .bind(award.linked_vault_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;

        Ok(ScholarshipReviewDecisionResult { application, award, pool })
    }

    async fn transition_award_with_audit(
        &self,
        award_id: Uuid,
        status: &str,
        linked_payout_request_id: Option<Uuid>,
        linked_vault_id: Option<Uuid>,
        pool_available_funds_minor: i64,
        audit_log: AuditLog,
    ) -> Result<ScholarshipReviewDecisionResult, PersistenceError> {
        let mut tx = self.pool.begin().await.map_err(map_err)?;
        let award: ScholarshipAward = sqlx::query_as::<_, ScholarshipAwardRow>(
            r#"UPDATE scholarship_awards
               SET status = $2,
                   linked_payout_request_id = COALESCE($3, linked_payout_request_id),
                   linked_vault_id = COALESCE($4, linked_vault_id),
                   updated_at = NOW()
               WHERE id = $1
               RETURNING id, application_id, decided_by, amount_minor, currency, status, decision_notes, linked_payout_request_id, linked_vault_id, created_at, updated_at"#,
        )
        .bind(award_id)
        .bind(status)
        .bind(linked_payout_request_id)
        .bind(linked_vault_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        let application: ScholarshipApplication = sqlx::query_as::<_, ScholarshipApplicationRow>(
            r#"SELECT id, pool_id, applicant_user_id, child_profile_id, student_country, education_level, school_id, category, status, notes, created_at, updated_at
               FROM scholarship_applications WHERE id = $1"#,
        )
        .bind(award.application_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        let pool = sqlx::query_as::<_, ScholarshipPoolRow>(
            r#"UPDATE scholarship_pools
               SET available_funds_minor = $2, updated_at = NOW()
               WHERE id = $1
               RETURNING id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction, created_at, updated_at"#,
        )
        .bind(application.pool_id)
        .bind(pool_available_funds_minor)
        .fetch_one(tx.as_mut())
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)?;

        append_audit_log(&mut tx, audit_log).await?;
        tx.commit().await.map_err(map_err)?;
        Ok(ScholarshipReviewDecisionResult { application, award, pool })
    }
}

#[async_trait]
impl AuditLogRepository for PgAuditLogRepository {
    async fn append(&self, audit_log: AuditLog) -> Result<AuditLog, PersistenceError> {
        let audit_log = sanitize_audit_log(audit_log);
        tracing::info!(
            target: "audit",
            audit_action = %audit_log.action,
            entity_type = %audit_log.entity_type,
            entity_id = ?audit_log.entity_id,
            actor_user_id = ?audit_log.actor_user_id,
            request_id = audit_log.request_id.as_deref().unwrap_or("unknown"),
            correlation_id = audit_log.correlation_id.as_deref().unwrap_or("unknown"),
            metadata = %audit_log.metadata,
            "audit event recorded"
        );
        sqlx::query_as::<_, AuditLogRow>(
            r#"INSERT INTO audit_logs (id, actor_user_id, entity_type, entity_id, action, request_id, correlation_id, metadata)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
               RETURNING id, actor_user_id, entity_type, entity_id, action, request_id, correlation_id, metadata, created_at, updated_at"#,
        )
        .bind(audit_log.id)
        .bind(audit_log.actor_user_id)
        .bind(audit_log.entity_type)
        .bind(audit_log.entity_id)
        .bind(audit_log.action)
        .bind(audit_log.request_id)
        .bind(audit_log.correlation_id)
        .bind(audit_log.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .map(Into::into)
    }

    async fn list_by_entity(
        &self,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<AuditLog>, PersistenceError> {
        sqlx::query_as::<_, AuditLogRow>(
            r#"SELECT id, actor_user_id, entity_type, entity_id, action, request_id, correlation_id, metadata, created_at, updated_at
               FROM audit_logs WHERE entity_type = $1 AND entity_id = $2 ORDER BY created_at DESC"#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    async fn list_by_actor(&self, actor_user_id: Uuid) -> Result<Vec<AuditLog>, PersistenceError> {
        sqlx::query_as::<_, AuditLogRow>(
            r#"SELECT id, actor_user_id, entity_type, entity_id, action, request_id, correlation_id, metadata, created_at, updated_at
               FROM audit_logs WHERE actor_user_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(actor_user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }
}

#[async_trait]
impl NotificationRepository for PgNotificationRepository {
    async fn create(&self, notification: Notification) -> Result<Notification, PersistenceError> {
        sqlx::query_as::<_, NotificationRow>(
            r#"INSERT INTO notifications
               (id, user_id, notification_type, title, body, metadata, status, read_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
               RETURNING id, user_id, notification_type, title, body, metadata, status, read_at, created_at, updated_at"#,
        )
        .bind(notification.id)
        .bind(notification.user_id)
        .bind(notification.notification_type.as_str())
        .bind(notification.title)
        .bind(notification.body)
        .bind(notification.metadata)
        .bind(notification.status.as_str())
        .bind(notification.read_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Notification>, PersistenceError> {
        sqlx::query_as::<_, NotificationRow>(
            r#"SELECT
                    id, user_id, notification_type, title, body, metadata, status, read_at, created_at, updated_at
               FROM notifications
               WHERE id = $1"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<Notification>, PersistenceError> {
        sqlx::query_as::<_, NotificationRow>(
            r#"SELECT
                    id, user_id, notification_type, title, body, metadata, status, read_at, created_at, updated_at
               FROM notifications WHERE user_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn mark_read(&self, id: Uuid) -> Result<(), PersistenceError> {
        sqlx::query(r#"UPDATE notifications SET status = 'read', read_at = NOW() WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    async fn mark_unread(&self, id: Uuid) -> Result<(), PersistenceError> {
        sqlx::query(r#"UPDATE notifications SET status = 'pending', read_at = NULL WHERE id = $1"#)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_err)?;
        Ok(())
    }
}

#[async_trait]
impl NotificationPreferenceRepository for PgNotificationPreferenceRepository {
    async fn upsert(
        &self,
        preference: NotificationPreference,
    ) -> Result<NotificationPreference, PersistenceError> {
        sqlx::query_as::<_, NotificationPreferenceRow>(
            r#"INSERT INTO notification_preferences
               (id, user_id, notification_type, in_app_enabled, email_enabled, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7)
               ON CONFLICT (user_id, notification_type) DO UPDATE
               SET in_app_enabled = EXCLUDED.in_app_enabled,
                   email_enabled = EXCLUDED.email_enabled,
                   updated_at = EXCLUDED.updated_at
               RETURNING
                    id, user_id, notification_type, in_app_enabled, email_enabled, created_at, updated_at"#,
        )
        .bind(preference.id)
        .bind(preference.user_id)
        .bind(preference.notification_type.as_str())
        .bind(preference.in_app_enabled)
        .bind(preference.email_enabled)
        .bind(preference.created_at)
        .bind(preference.updated_at)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_user_and_type(
        &self,
        user_id: Uuid,
        notification_type: &str,
    ) -> Result<Option<NotificationPreference>, PersistenceError> {
        sqlx::query_as::<_, NotificationPreferenceRow>(
            r#"SELECT
                    id, user_id, notification_type, in_app_enabled, email_enabled, created_at, updated_at
               FROM notification_preferences
               WHERE user_id = $1 AND notification_type = $2"#,
        )
        .bind(user_id)
        .bind(notification_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn list_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<NotificationPreference>, PersistenceError> {
        sqlx::query_as::<_, NotificationPreferenceRow>(
            r#"SELECT
                    id, user_id, notification_type, in_app_enabled, email_enabled, created_at, updated_at
               FROM notification_preferences
               WHERE user_id = $1
               ORDER BY notification_type ASC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }
}

#[async_trait]
impl WalletAccountRepository for PgWalletAccountRepository {
    async fn create(&self, wallet: WalletAccount) -> Result<WalletAccount, PersistenceError> {
        sqlx::query_as::<_, WalletAccountRow>(
            r#"INSERT INTO wallet_accounts (id, user_id, network, address, label)
               VALUES ($1,$2,$3,$4,$5)
               RETURNING id, user_id, network, address, label, created_at, updated_at"#,
        )
        .bind(wallet.id)
        .bind(wallet.user_id)
        .bind(wallet.network)
        .bind(wallet.address)
        .bind(wallet.label)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .map(Into::into)
    }

    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<WalletAccount>, PersistenceError> {
        sqlx::query_as::<_, WalletAccountRow>(
            r#"SELECT id, user_id, network, address, label, created_at, updated_at
               FROM wallet_accounts WHERE user_id = $1 ORDER BY created_at DESC"#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }
}

#[async_trait]
impl ExternalReferenceRepository for PgExternalReferenceRepository {
    async fn create(
        &self,
        reference: ExternalReference,
    ) -> Result<ExternalReference, PersistenceError> {
        sqlx::query_as::<_, ExternalReferenceRow>(
            r#"INSERT INTO external_references
               (id, entity_type, entity_id, reference_kind, value, metadata)
               VALUES ($1,$2,$3,$4,$5,$6)
               RETURNING id, entity_type, entity_id, reference_kind, value, metadata, created_at, updated_at"#,
        )
        .bind(reference.id)
        .bind(reference.entity_type.as_str())
        .bind(reference.entity_id)
        .bind(reference.reference_kind.as_str())
        .bind(reference.value)
        .bind(reference.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_entity(
        &self,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<ExternalReference>, PersistenceError> {
        sqlx::query_as::<_, ExternalReferenceRow>(
            r#"SELECT id, entity_type, entity_id, reference_kind, value, metadata, created_at, updated_at
               FROM external_references
               WHERE entity_type = $1 AND entity_id = $2
               ORDER BY created_at ASC"#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_err)?
        .into_iter()
        .map(TryInto::try_into)
        .collect()
    }

    async fn find_one(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        reference_kind: &str,
    ) -> Result<Option<ExternalReference>, PersistenceError> {
        sqlx::query_as::<_, ExternalReferenceRow>(
            r#"SELECT id, entity_type, entity_id, reference_kind, value, metadata, created_at, updated_at
               FROM external_references
               WHERE entity_type = $1 AND entity_id = $2 AND reference_kind = $3"#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .bind(reference_kind)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }
}

#[async_trait]
impl BlockchainTransactionRepository for PgBlockchainTransactionRepository {
    async fn create(
        &self,
        record: BlockchainTransactionRecord,
    ) -> Result<BlockchainTransactionRecord, PersistenceError> {
        sqlx::query_as::<_, BlockchainTransactionRecordRow>(
            r#"INSERT INTO blockchain_transaction_records
               (id, entity_type, entity_id, operation_kind, idempotency_key, status, tx_hash, attempt_count, last_error_code, last_error_message, next_retry_at, metadata)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
               RETURNING id, entity_type, entity_id, operation_kind, idempotency_key, status, tx_hash, attempt_count, last_error_code, last_error_message, next_retry_at, metadata, created_at, updated_at"#,
        )
        .bind(record.id)
        .bind(record.entity_type.as_str())
        .bind(record.entity_id)
        .bind(record.operation_kind)
        .bind(record.idempotency_key)
        .bind(record.status.as_str())
        .bind(record.tx_hash)
        .bind(record.attempt_count)
        .bind(record.last_error_code)
        .bind(record.last_error_message)
        .bind(record.next_retry_at)
        .bind(record.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn update(
        &self,
        record: BlockchainTransactionRecord,
    ) -> Result<BlockchainTransactionRecord, PersistenceError> {
        sqlx::query_as::<_, BlockchainTransactionRecordRow>(
            r#"UPDATE blockchain_transaction_records
               SET status = $2,
                   tx_hash = $3,
                   attempt_count = $4,
                   last_error_code = $5,
                   last_error_message = $6,
                   next_retry_at = $7,
                   metadata = $8,
                   updated_at = NOW()
               WHERE id = $1
               RETURNING id, entity_type, entity_id, operation_kind, idempotency_key, status, tx_hash, attempt_count, last_error_code, last_error_message, next_retry_at, metadata, created_at, updated_at"#,
        )
        .bind(record.id)
        .bind(record.status.as_str())
        .bind(record.tx_hash)
        .bind(record.attempt_count)
        .bind(record.last_error_code)
        .bind(record.last_error_message)
        .bind(record.next_retry_at)
        .bind(record.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(map_err)
        .and_then(TryInto::try_into)
    }

    async fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<BlockchainTransactionRecord>, PersistenceError> {
        sqlx::query_as::<_, BlockchainTransactionRecordRow>(
            r#"SELECT id, entity_type, entity_id, operation_kind, idempotency_key, status, tx_hash, attempt_count, last_error_code, last_error_message, next_retry_at, metadata, created_at, updated_at
               FROM blockchain_transaction_records
               WHERE idempotency_key = $1"#,
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }

    async fn find_by_entity(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        operation_kind: &str,
    ) -> Result<Option<BlockchainTransactionRecord>, PersistenceError> {
        sqlx::query_as::<_, BlockchainTransactionRecordRow>(
            r#"SELECT id, entity_type, entity_id, operation_kind, idempotency_key, status, tx_hash, attempt_count, last_error_code, last_error_message, next_retry_at, metadata, created_at, updated_at
               FROM blockchain_transaction_records
               WHERE entity_type = $1 AND entity_id = $2 AND operation_kind = $3"#,
        )
        .bind(entity_type)
        .bind(entity_id)
        .bind(operation_kind)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_err)?
        .map(TryInto::try_into)
        .transpose()
    }
}

fn parse_money(amount_minor: i64, currency: &str) -> Result<Money, PersistenceError> {
    let currency =
        currency.parse::<Currency>().map_err(|err| PersistenceError::Repository(err.to_owned()))?;
    Money::new(amount_minor, currency).map_err(|err| PersistenceError::Repository(err.to_owned()))
}

fn parse_status<T>(value: &str) -> Result<T, PersistenceError>
where
    T: std::str::FromStr<Err = &'static str>,
{
    value.parse::<T>().map_err(|err| PersistenceError::Repository(err.to_owned()))
}

fn map_err(err: sqlx::Error) -> PersistenceError {
    match &err {
        sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some("23505") => {
            PersistenceError::Conflict(db_err.message().to_owned())
        }
        _ => PersistenceError::Repository(err.to_string()),
    }
}

fn push_where(builder: &mut QueryBuilder<'_, Postgres>, has_where: &mut bool, column: &str) {
    if *has_where {
        builder.push(" AND ");
    } else {
        builder.push(" WHERE ");
        *has_where = true;
    }

    builder.push(column).push(" = ");
}

fn sanitize_audit_log(mut audit_log: AuditLog) -> AuditLog {
    if let Some(context) = application::audit::current_audit_context() {
        if audit_log.request_id.is_none() {
            audit_log.request_id = Some(context.request_id);
        }
        if audit_log.correlation_id.is_none() {
            audit_log.correlation_id = Some(context.correlation_id);
        }
    }
    audit_log.metadata = sanitize_json_value(audit_log.metadata);
    audit_log
}

fn sanitize_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(key, value)| {
                    let sanitized = if is_sensitive_key(&key) {
                        Value::String("[REDACTED]".to_owned())
                    } else {
                        sanitize_json_value(value)
                    };
                    (key, sanitized)
                })
                .collect(),
        ),
        Value::Array(items) => {
            Value::Array(items.into_iter().map(sanitize_json_value).collect::<Vec<_>>())
        }
        other => other,
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    normalized.contains("password")
        || normalized.contains("secret")
        || normalized.contains("token")
        || normalized.contains("authorization")
        || normalized.contains("payout_reference")
        || normalized.contains("provider_reference")
        || normalized == "external_reference"
        || normalized == "decision_notes"
        || normalized == "review_notes"
}

async fn append_audit_log(
    tx: &mut Transaction<'_, Postgres>,
    audit_log: AuditLog,
) -> Result<(), PersistenceError> {
    let audit_log = sanitize_audit_log(audit_log);
    tracing::info!(
        target: "audit",
        audit_action = %audit_log.action,
        entity_type = %audit_log.entity_type,
        entity_id = ?audit_log.entity_id,
        actor_user_id = ?audit_log.actor_user_id,
        request_id = audit_log.request_id.as_deref().unwrap_or("unknown"),
        correlation_id = audit_log.correlation_id.as_deref().unwrap_or("unknown"),
        metadata = %audit_log.metadata,
        "audit event recorded"
    );
    sqlx::query(
        r#"INSERT INTO audit_logs (id, actor_user_id, entity_type, entity_id, action, request_id, correlation_id, metadata)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
    )
    .bind(audit_log.id)
    .bind(audit_log.actor_user_id)
    .bind(audit_log.entity_type)
    .bind(audit_log.entity_id)
    .bind(audit_log.action)
    .bind(audit_log.request_id)
    .bind(audit_log.correlation_id)
    .bind(audit_log.metadata)
    .execute(tx.as_mut())
    .await
    .map_err(map_err)?;

    Ok(())
}

async fn find_contribution_for_update(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Option<ContributionRow>, PersistenceError> {
    sqlx::query_as::<_, ContributionRow>(
        r#"SELECT id, vault_id, contributor_user_id, amount_minor, currency, status, source_type, external_reference, idempotency_key, created_at, updated_at
           FROM contributions
           WHERE id = $1
           FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_err)
}

async fn find_vault_for_update(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Option<SavingsVault>, PersistenceError> {
    let row = sqlx::query_as::<_, SavingsVaultRow>(
        r#"SELECT id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, total_disbursed_minor, external_wallet_account_id, external_contract_ref, version, created_at, updated_at
           FROM savings_vaults
           WHERE id = $1
           FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_err)?;

    row.map(TryInto::try_into).transpose()
}

async fn update_vault_totals(
    tx: &mut Transaction<'_, Postgres>,
    vault: &SavingsVault,
    total_contributed_minor: i64,
) -> Result<SavingsVault, PersistenceError> {
    let row = sqlx::query_as::<_, SavingsVaultRow>(
        r#"UPDATE savings_vaults
           SET total_contributed_minor = $2, version = version + 1, updated_at = NOW()
           WHERE id = $1
           RETURNING id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, total_disbursed_minor, external_wallet_account_id, external_contract_ref, version, created_at, updated_at"#,
    )
    .bind(vault.id)
    .bind(total_contributed_minor)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_err)?;

    row.try_into()
}

async fn update_vault_locked_and_disbursed(
    tx: &mut Transaction<'_, Postgres>,
    vault: &SavingsVault,
    total_locked_minor: i64,
    total_disbursed_minor: i64,
) -> Result<SavingsVault, PersistenceError> {
    let row = sqlx::query_as::<_, SavingsVaultRow>(
        r#"UPDATE savings_vaults
           SET total_locked_minor = $2,
               total_disbursed_minor = $3,
               version = version + 1,
               updated_at = NOW()
           WHERE id = $1
           RETURNING id, plan_id, owner_user_id, currency, status, total_contributed_minor, total_locked_minor, total_disbursed_minor, external_wallet_account_id, external_contract_ref, version, created_at, updated_at"#,
    )
    .bind(vault.id)
    .bind(total_locked_minor)
    .bind(total_disbursed_minor)
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_err)?;

    row.try_into()
}

async fn find_payout_for_update(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Option<PayoutRequestRow>, PersistenceError> {
    sqlx::query_as::<_, PayoutRequestRow>(
        r#"SELECT id, vault_id, milestone_id, school_id, requested_by, amount_minor, currency, idempotency_key, status, review_notes, external_payout_reference, reviewed_by, reviewed_at, created_at, updated_at
           FROM payout_requests
           WHERE id = $1
           FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_err)
}

async fn find_scholarship_pool_for_update(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Option<ScholarshipPool>, PersistenceError> {
    let row = sqlx::query_as::<_, ScholarshipPoolRow>(
        r#"SELECT id, owner_user_id, name, description, status, available_funds_minor, currency, geography_restriction, education_level_restriction, school_id_restriction, category_restriction, created_at, updated_at
           FROM scholarship_pools
           WHERE id = $1
           FOR UPDATE"#,
    )
    .bind(id)
    .fetch_optional(tx.as_mut())
    .await
    .map_err(map_err)?;

    row.map(TryInto::try_into).transpose()
}

async fn insert_ledger_entry(
    tx: &mut Transaction<'_, Postgres>,
    vault: &SavingsVault,
    contribution: &Contribution,
    actor_user_id: Uuid,
    entry_type: VaultLedgerEntryType,
    external_reference: Option<&str>,
) -> Result<VaultLedgerEntry, PersistenceError> {
    let row = sqlx::query_as::<_, VaultLedgerEntryRow>(
        r#"INSERT INTO vault_ledger_entries
           (id, vault_id, contribution_id, actor_user_id, entry_type, amount_minor, currency, balance_after_minor, external_reference, metadata)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
           RETURNING id, vault_id, contribution_id, actor_user_id, entry_type, amount_minor, currency, balance_after_minor, external_reference, metadata, created_at, updated_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(vault.id)
    .bind(contribution.id)
    .bind(actor_user_id)
    .bind(entry_type.as_str())
    .bind(contribution.amount.amount_minor)
    .bind(contribution.amount.currency.to_string())
    .bind(vault.total_contributed_minor)
    .bind(external_reference)
    .bind(serde_json::json!({
        "contribution_status": contribution.status.as_str(),
    }))
    .fetch_one(tx.as_mut())
    .await
    .map_err(map_err)?;

    row.try_into()
}

impl TryFrom<ChildProfileRow> for ChildProfile {
    type Error = PersistenceError;
    fn try_from(v: ChildProfileRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            owner_user_id: v.owner_user_id,
            full_name: v.full_name,
            date_of_birth: v.date_of_birth,
            education_level: v.education_level,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<SavingsPlanRow> for SavingsPlan {
    type Error = PersistenceError;
    fn try_from(v: SavingsPlanRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            child_profile_id: v.child_profile_id,
            owner_user_id: v.owner_user_id,
            name: v.name,
            description: v.description,
            target_amount: parse_money(v.target_amount_minor, &v.target_currency)?,
            status: parse_status(&v.status)?,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<SavingsVaultRow> for SavingsVault {
    type Error = PersistenceError;
    fn try_from(v: SavingsVaultRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            plan_id: v.plan_id,
            owner_user_id: v.owner_user_id,
            currency: v.currency,
            status: parse_status(&v.status)?,
            total_contributed_minor: v.total_contributed_minor,
            total_locked_minor: v.total_locked_minor,
            total_disbursed_minor: v.total_disbursed_minor,
            external_wallet_account_id: v.external_wallet_account_id,
            external_contract_ref: v.external_contract_ref,
            version: v.version,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl From<VaultContributorRow> for VaultContributor {
    fn from(v: VaultContributorRow) -> Self {
        Self {
            id: v.id,
            vault_id: v.vault_id,
            contributor_user_id: v.contributor_user_id,
            role_label: v.role_label,
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}

impl TryFrom<MilestoneRow> for Milestone {
    type Error = PersistenceError;
    fn try_from(v: MilestoneRow) -> Result<Self, Self::Error> {
        let target_amount = parse_money(v.target_amount_minor, &v.currency)?;
        let funded_amount = parse_money(v.funded_amount_minor, &v.currency)?;
        Ok(Self {
            id: v.id,
            vault_id: v.vault_id,
            title: v.title,
            description: v.description,
            due_date: v.due_date,
            target_amount,
            funded_amount,
            payout_type: parse_status(&v.payout_type)?,
            status: parse_status(&v.status)?,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<ContributionRow> for Contribution {
    type Error = PersistenceError;
    fn try_from(v: ContributionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            vault_id: v.vault_id,
            contributor_user_id: v.contributor_user_id,
            amount: parse_money(v.amount_minor, &v.currency)?,
            status: parse_status(&v.status)?,
            source_type: parse_status(&v.source_type)?,
            external_reference: v.external_reference,
            idempotency_key: v.idempotency_key,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<VaultLedgerEntryRow> for VaultLedgerEntry {
    type Error = PersistenceError;

    fn try_from(v: VaultLedgerEntryRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            vault_id: v.vault_id,
            contribution_id: v.contribution_id,
            actor_user_id: v.actor_user_id,
            entry_type: parse_status(&v.entry_type)?,
            amount: parse_money(v.amount_minor, &v.currency)?,
            balance_after_minor: v.balance_after_minor,
            external_reference: v.external_reference,
            metadata: v.metadata,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<SchoolRow> for School {
    type Error = PersistenceError;

    fn try_from(v: SchoolRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            legal_name: v.legal_name,
            display_name: v.display_name,
            country: v.country,
            payout_method: parse_status(&v.payout_method)?,
            payout_reference: v.payout_reference,
            verification_status: parse_status(&v.verification_status)?,
            verified_by: v.verified_by,
            verified_at: v.verified_at,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<PayoutRequestRow> for PayoutRequest {
    type Error = PersistenceError;
    fn try_from(v: PayoutRequestRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            vault_id: v.vault_id,
            milestone_id: v.milestone_id,
            school_id: v.school_id,
            requested_by: v.requested_by,
            amount: parse_money(v.amount_minor, &v.currency)?,
            idempotency_key: v.idempotency_key,
            status: parse_status(&v.status)?,
            review_notes: v.review_notes,
            external_payout_reference: v.external_payout_reference,
            reviewed_by: v.reviewed_by,
            reviewed_at: v.reviewed_at,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<KycProfileRow> for KycProfile {
    type Error = PersistenceError;
    fn try_from(v: KycProfileRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            user_id: v.user_id,
            status: parse_status(&v.status)?,
            provider_reference: v.provider_reference,
            reviewed_at: v.reviewed_at,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<ScholarshipPoolRow> for ScholarshipPool {
    type Error = PersistenceError;
    fn try_from(v: ScholarshipPoolRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            owner_user_id: v.owner_user_id,
            name: v.name,
            description: v.description,
            status: parse_status::<ScholarshipPoolStatus>(&v.status)?,
            available_funds: parse_money(v.available_funds_minor, &v.currency)?,
            geography_restriction: v.geography_restriction,
            education_level_restriction: v.education_level_restriction,
            school_id_restriction: v.school_id_restriction,
            category_restriction: v.category_restriction,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<ScholarshipApplicationRow> for ScholarshipApplication {
    type Error = PersistenceError;
    fn try_from(v: ScholarshipApplicationRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            pool_id: v.pool_id,
            applicant_user_id: v.applicant_user_id,
            child_profile_id: v.child_profile_id,
            student_country: v.student_country,
            education_level: v.education_level,
            school_id: v.school_id,
            category: v.category,
            status: parse_status::<ScholarshipApplicationStatus>(&v.status)?,
            notes: v.notes,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<ScholarshipAwardRow> for ScholarshipAward {
    type Error = PersistenceError;
    fn try_from(v: ScholarshipAwardRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            application_id: v.application_id,
            decided_by: v.decided_by,
            amount: parse_money(v.amount_minor, &v.currency)?,
            status: parse_status::<ScholarshipAwardStatus>(&v.status)?,
            decision_notes: v.decision_notes,
            linked_payout_request_id: v.linked_payout_request_id,
            linked_vault_id: v.linked_vault_id,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<AchievementCredentialRow> for AchievementCredential {
    type Error = PersistenceError;
    fn try_from(v: AchievementCredentialRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            credential_ref: v.credential_ref,
            child_profile_id: v.child_profile_id,
            recipient_user_id: v.recipient_user_id,
            school_id: v.school_id,
            achievement_type: parse_status::<AchievementCredentialType>(&v.achievement_type)?,
            status: parse_status::<AchievementCredentialStatus>(&v.status)?,
            title: v.title,
            description: v.description,
            achievement_date: v.achievement_date,
            issued_by_user_id: v.issued_by_user_id,
            issued_by_role: v.issued_by_role,
            issuance_notes: v.issuance_notes,
            evidence_uri: v.evidence_uri,
            attestation_hash: v.attestation_hash,
            attestation_method: v.attestation_method,
            attestation_anchor: v.attestation_anchor,
            attestation_anchor_network: v.attestation_anchor_network,
            metadata: v.metadata,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<DonorContributionRow> for DonorContribution {
    type Error = PersistenceError;
    fn try_from(v: DonorContributionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            pool_id: v.pool_id,
            donor_user_id: v.donor_user_id,
            amount: parse_money(v.amount_minor, &v.currency)?,
            status: parse_status(&v.status)?,
            external_reference: v.external_reference,
            idempotency_key: v.idempotency_key,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl From<AuditLogRow> for AuditLog {
    fn from(v: AuditLogRow) -> Self {
        Self {
            id: v.id,
            actor_user_id: v.actor_user_id,
            entity_type: v.entity_type,
            entity_id: v.entity_id,
            action: v.action,
            request_id: v.request_id,
            correlation_id: v.correlation_id,
            metadata: v.metadata,
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}

impl TryFrom<NotificationRow> for Notification {
    type Error = PersistenceError;
    fn try_from(v: NotificationRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            user_id: v.user_id,
            notification_type: parse_status(&v.notification_type)?,
            title: v.title,
            body: v.body,
            metadata: v.metadata,
            status: parse_status(&v.status)?,
            read_at: v.read_at,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<NotificationPreferenceRow> for NotificationPreference {
    type Error = PersistenceError;

    fn try_from(v: NotificationPreferenceRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            user_id: v.user_id,
            notification_type: parse_status(&v.notification_type)?,
            in_app_enabled: v.in_app_enabled,
            email_enabled: v.email_enabled,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl From<WalletAccountRow> for WalletAccount {
    fn from(v: WalletAccountRow) -> Self {
        Self {
            id: v.id,
            user_id: v.user_id,
            network: v.network,
            address: v.address,
            label: v.label,
            created_at: v.created_at,
            updated_at: v.updated_at,
        }
    }
}

impl TryFrom<ExternalReferenceRow> for ExternalReference {
    type Error = PersistenceError;

    fn try_from(v: ExternalReferenceRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            entity_type: parse_status::<ExternalReferenceEntityType>(&v.entity_type)?,
            entity_id: v.entity_id,
            reference_kind: parse_status::<ExternalReferenceKind>(&v.reference_kind)?,
            value: v.value,
            metadata: v.metadata,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}

impl TryFrom<BlockchainTransactionRecordRow> for BlockchainTransactionRecord {
    type Error = PersistenceError;

    fn try_from(v: BlockchainTransactionRecordRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: v.id,
            entity_type: parse_status::<ExternalReferenceEntityType>(&v.entity_type)?,
            entity_id: v.entity_id,
            operation_kind: v.operation_kind,
            idempotency_key: v.idempotency_key,
            status: parse_status::<BlockchainTransactionStatus>(&v.status)?,
            tx_hash: v.tx_hash,
            attempt_count: v.attempt_count,
            last_error_code: v.last_error_code,
            last_error_message: v.last_error_message,
            next_retry_at: v.next_retry_at,
            metadata: v.metadata,
            created_at: v.created_at,
            updated_at: v.updated_at,
        })
    }
}
