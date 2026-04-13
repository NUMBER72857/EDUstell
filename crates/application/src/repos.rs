use async_trait::async_trait;
use domain::{
    contributions::VaultLedgerEntry,
    persistence::{
        AchievementCredential, AuditLog, BlockchainTransactionRecord, ChildProfile, Contribution,
        DonorContribution, ExternalReference, KycProfile, Milestone, Notification,
        NotificationPreference, PayoutRequest, SavingsPlan, SavingsVault, ScholarshipApplication,
        ScholarshipAward, ScholarshipPool, School, VaultContributor, WalletAccount,
    },
};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("validation: {0}")]
    Validation(String),
    #[error("repository: {0}")]
    Repository(String),
}

#[async_trait]
pub trait ChildProfileRepository: Send + Sync {
    async fn create(&self, profile: ChildProfile) -> Result<ChildProfile, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ChildProfile>, PersistenceError>;
    async fn list_by_owner(
        &self,
        owner_user_id: Uuid,
    ) -> Result<Vec<ChildProfile>, PersistenceError>;
}

#[async_trait]
pub trait SavingsPlanRepository: Send + Sync {
    async fn create(&self, plan: SavingsPlan) -> Result<SavingsPlan, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<SavingsPlan>, PersistenceError>;
    async fn list_by_child_profile(
        &self,
        child_profile_id: Uuid,
    ) -> Result<Vec<SavingsPlan>, PersistenceError>;
}

#[async_trait]
pub trait VaultRepository: Send + Sync {
    async fn create(&self, vault: SavingsVault) -> Result<SavingsVault, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<SavingsVault>, PersistenceError>;
    async fn update_balances(
        &self,
        id: Uuid,
        total_contributed_minor: i64,
        total_locked_minor: i64,
        expected_version: i64,
    ) -> Result<(), PersistenceError>;
    async fn add_contributor(
        &self,
        contributor: VaultContributor,
    ) -> Result<VaultContributor, PersistenceError>;
    async fn list_contributors(
        &self,
        vault_id: Uuid,
    ) -> Result<Vec<VaultContributor>, PersistenceError>;
}

#[async_trait]
pub trait MilestoneRepository: Send + Sync {
    async fn create(&self, milestone: Milestone) -> Result<Milestone, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Milestone>, PersistenceError>;
    async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<Milestone>, PersistenceError>;
    async fn update_funded_amount(
        &self,
        id: Uuid,
        funded_amount_minor: i64,
        status: &str,
    ) -> Result<(), PersistenceError>;
}

#[async_trait]
pub trait ContributionRepository: Send + Sync {
    async fn create(&self, contribution: Contribution) -> Result<Contribution, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Contribution>, PersistenceError>;
    async fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<Contribution>, PersistenceError>;
    async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<Contribution>, PersistenceError>;
    async fn update_status(
        &self,
        id: Uuid,
        status: &str,
        external_reference: Option<&str>,
    ) -> Result<(), PersistenceError>;
}

#[async_trait]
pub trait VaultLedgerRepository: Send + Sync {
    async fn list_by_vault(
        &self,
        vault_id: Uuid,
    ) -> Result<Vec<VaultLedgerEntry>, PersistenceError>;
}

#[derive(Debug, Clone)]
pub struct ContributionSettlementResult {
    pub contribution: Contribution,
    pub ledger_entry: Option<VaultLedgerEntry>,
    pub vault: SavingsVault,
}

#[async_trait]
pub trait ContributionWorkflowRepository: Send + Sync {
    async fn create_intent_with_audit(
        &self,
        contribution: Contribution,
        audit_log: AuditLog,
    ) -> Result<Contribution, PersistenceError>;

    async fn settle_with_audit(
        &self,
        contribution_id: Uuid,
        status: &str,
        actor_user_id: Uuid,
        external_reference: Option<&str>,
        audit_log: AuditLog,
    ) -> Result<ContributionSettlementResult, PersistenceError>;
}

#[async_trait]
pub trait SchoolRepository: Send + Sync {
    async fn create(&self, school: School) -> Result<School, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<School>, PersistenceError>;
    async fn search_verified(&self, query: Option<&str>) -> Result<Vec<School>, PersistenceError>;
    async fn list_verified(&self) -> Result<Vec<School>, PersistenceError>;
    async fn update_verification(
        &self,
        id: Uuid,
        verification_status: &str,
        verified_by: Option<Uuid>,
        verified_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), PersistenceError>;
}

#[async_trait]
pub trait PayoutRepository: Send + Sync {
    async fn create(&self, payout: PayoutRequest) -> Result<PayoutRequest, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<PayoutRequest>, PersistenceError>;
    async fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<PayoutRequest>, PersistenceError>;
    async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<PayoutRequest>, PersistenceError>;
    async fn update_status(
        &self,
        id: Uuid,
        status: &str,
        review_notes: Option<&str>,
        external_payout_reference: Option<&str>,
    ) -> Result<(), PersistenceError>;
}

#[async_trait]
pub trait SchoolWorkflowRepository: Send + Sync {
    async fn create_with_audit(
        &self,
        school: School,
        audit_log: AuditLog,
    ) -> Result<School, PersistenceError>;

    async fn verify_with_audit(
        &self,
        school_id: Uuid,
        verification_status: &str,
        verified_by: Uuid,
        audit_log: AuditLog,
    ) -> Result<School, PersistenceError>;
}

#[derive(Debug, Clone)]
pub struct PayoutTransitionResult {
    pub payout: PayoutRequest,
    pub vault: SavingsVault,
}

#[async_trait]
pub trait PayoutWorkflowRepository: Send + Sync {
    async fn create_request_with_audit(
        &self,
        payout: PayoutRequest,
        audit_log: AuditLog,
    ) -> Result<PayoutRequest, PersistenceError>;

    async fn transition_with_audit(
        &self,
        payout_id: Uuid,
        status: &str,
        actor_user_id: Uuid,
        review_notes: Option<&str>,
        external_payout_reference: Option<&str>,
        audit_log: AuditLog,
    ) -> Result<PayoutTransitionResult, PersistenceError>;
}

#[async_trait]
pub trait KycRepository: Send + Sync {
    async fn upsert(&self, profile: KycProfile) -> Result<KycProfile, PersistenceError>;
    async fn find_by_user(&self, user_id: Uuid) -> Result<Option<KycProfile>, PersistenceError>;
}

#[async_trait]
pub trait ScholarshipPoolRepository: Send + Sync {
    async fn create(&self, pool: ScholarshipPool) -> Result<ScholarshipPool, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScholarshipPool>, PersistenceError>;
    async fn list(&self) -> Result<Vec<ScholarshipPool>, PersistenceError>;
}

#[async_trait]
pub trait ScholarshipApplicationRepository: Send + Sync {
    async fn create(
        &self,
        application: ScholarshipApplication,
    ) -> Result<ScholarshipApplication, PersistenceError>;
    async fn list_by_pool(
        &self,
        pool_id: Uuid,
    ) -> Result<Vec<ScholarshipApplication>, PersistenceError>;
    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<ScholarshipApplication>, PersistenceError>;
}

#[async_trait]
pub trait ScholarshipAwardRepository: Send + Sync {
    async fn create(&self, award: ScholarshipAward) -> Result<ScholarshipAward, PersistenceError>;
    async fn list_by_pool(&self, pool_id: Uuid) -> Result<Vec<ScholarshipAward>, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ScholarshipAward>, PersistenceError>;
}

#[async_trait]
pub trait AchievementCredentialRepository: Send + Sync {
    async fn create_with_audit(
        &self,
        credential: AchievementCredential,
        audit_log: AuditLog,
    ) -> Result<AchievementCredential, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<AchievementCredential>, PersistenceError>;
    async fn list_by_child_profile(
        &self,
        child_profile_id: Uuid,
    ) -> Result<Vec<AchievementCredential>, PersistenceError>;
    async fn list_by_recipient(
        &self,
        recipient_user_id: Uuid,
    ) -> Result<Vec<AchievementCredential>, PersistenceError>;
    async fn list_by_issuer(
        &self,
        issued_by_user_id: Uuid,
    ) -> Result<Vec<AchievementCredential>, PersistenceError>;
    async fn list_recent(
        &self,
        limit: i64,
    ) -> Result<Vec<AchievementCredential>, PersistenceError>;
}

#[async_trait]
pub trait DonorContributionRepository: Send + Sync {
    async fn create(
        &self,
        contribution: DonorContribution,
    ) -> Result<DonorContribution, PersistenceError>;
    async fn list_by_pool(&self, pool_id: Uuid)
    -> Result<Vec<DonorContribution>, PersistenceError>;
}

#[derive(Debug, Clone)]
pub struct ScholarshipReviewDecisionResult {
    pub application: ScholarshipApplication,
    pub award: ScholarshipAward,
    pub pool: ScholarshipPool,
}

#[async_trait]
pub trait ScholarshipWorkflowRepository: Send + Sync {
    async fn create_pool_with_audit(
        &self,
        pool: ScholarshipPool,
        audit_log: AuditLog,
    ) -> Result<ScholarshipPool, PersistenceError>;

    async fn fund_pool_with_audit(
        &self,
        contribution: DonorContribution,
        audit_log: AuditLog,
    ) -> Result<(ScholarshipPool, DonorContribution), PersistenceError>;

    async fn create_application_with_audit(
        &self,
        application: ScholarshipApplication,
        audit_log: AuditLog,
    ) -> Result<ScholarshipApplication, PersistenceError>;

    async fn decide_application_with_audit(
        &self,
        application_id: Uuid,
        application_status: &str,
        award: ScholarshipAward,
        pool_available_funds_minor: i64,
        audit_log: AuditLog,
    ) -> Result<ScholarshipReviewDecisionResult, PersistenceError>;

    async fn transition_award_with_audit(
        &self,
        award_id: Uuid,
        status: &str,
        linked_payout_request_id: Option<Uuid>,
        linked_vault_id: Option<Uuid>,
        pool_available_funds_minor: i64,
        audit_log: AuditLog,
    ) -> Result<ScholarshipReviewDecisionResult, PersistenceError>;
}

#[async_trait]
pub trait AuditLogRepository: Send + Sync {
    async fn append(&self, audit_log: AuditLog) -> Result<AuditLog, PersistenceError>;
    async fn list_by_entity(
        &self,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<AuditLog>, PersistenceError>;
    async fn list_by_actor(&self, actor_user_id: Uuid) -> Result<Vec<AuditLog>, PersistenceError>;
}

#[async_trait]
pub trait NotificationRepository: Send + Sync {
    async fn create(&self, notification: Notification) -> Result<Notification, PersistenceError>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Notification>, PersistenceError>;
    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<Notification>, PersistenceError>;
    async fn mark_read(&self, id: Uuid) -> Result<(), PersistenceError>;
    async fn mark_unread(&self, id: Uuid) -> Result<(), PersistenceError>;
}

#[async_trait]
pub trait NotificationPreferenceRepository: Send + Sync {
    async fn upsert(
        &self,
        preference: NotificationPreference,
    ) -> Result<NotificationPreference, PersistenceError>;
    async fn find_by_user_and_type(
        &self,
        user_id: Uuid,
        notification_type: &str,
    ) -> Result<Option<NotificationPreference>, PersistenceError>;
    async fn list_by_user(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<NotificationPreference>, PersistenceError>;
}

#[async_trait]
pub trait WalletAccountRepository: Send + Sync {
    async fn create(&self, wallet: WalletAccount) -> Result<WalletAccount, PersistenceError>;
    async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<WalletAccount>, PersistenceError>;
}

#[async_trait]
pub trait ExternalReferenceRepository: Send + Sync {
    async fn create(
        &self,
        reference: ExternalReference,
    ) -> Result<ExternalReference, PersistenceError>;
    async fn find_by_entity(
        &self,
        entity_type: &str,
        entity_id: Uuid,
    ) -> Result<Vec<ExternalReference>, PersistenceError>;
    async fn find_one(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        reference_kind: &str,
    ) -> Result<Option<ExternalReference>, PersistenceError>;
}

#[async_trait]
pub trait BlockchainTransactionRepository: Send + Sync {
    async fn create(
        &self,
        record: BlockchainTransactionRecord,
    ) -> Result<BlockchainTransactionRecord, PersistenceError>;
    async fn update(
        &self,
        record: BlockchainTransactionRecord,
    ) -> Result<BlockchainTransactionRecord, PersistenceError>;
    async fn find_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<BlockchainTransactionRecord>, PersistenceError>;
    async fn find_by_entity(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        operation_kind: &str,
    ) -> Result<Option<BlockchainTransactionRecord>, PersistenceError>;
}

pub fn empty_metadata() -> Value {
    Value::Object(Default::default())
}
