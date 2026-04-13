use std::future::Future;

use chrono::{DateTime, Utc};
use domain::persistence::{
    AchievementCredential, AuditLog, Contribution, ExternalReference, KycProfile, Milestone,
    PayoutRequest, SavingsPlan, ScholarshipAward, ScholarshipPool, School, VaultContributor,
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::repos::{AuditLogRepository, PersistenceError};

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("not found")]
    NotFound,
    #[error("validation: {0}")]
    Validation(String),
    #[error("repository: {0}")]
    Repository(String),
}

impl From<PersistenceError> for AuditError {
    fn from(value: PersistenceError) -> Self {
        match value {
            PersistenceError::NotFound => Self::NotFound,
            PersistenceError::Validation(message) | PersistenceError::Conflict(message) => {
                Self::Validation(message)
            }
            PersistenceError::Repository(message) => Self::Repository(message),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEntityType {
    User,
    KycProfile,
    SavingsPlan,
    Milestone,
    VaultContributor,
    Contribution,
    PayoutRequest,
    School,
    ScholarshipAward,
    AchievementCredential,
    ExternalReference,
}

impl AuditEntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::KycProfile => "kyc_profile",
            Self::SavingsPlan => "savings_plan",
            Self::Milestone => "milestone",
            Self::VaultContributor => "vault_contributor",
            Self::Contribution => "contribution",
            Self::PayoutRequest => "payout_request",
            Self::School => "school",
            Self::ScholarshipAward => "scholarship_award",
            Self::AchievementCredential => "achievement_credential",
            Self::ExternalReference => "external_reference",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    UserRegistered,
    KycStatusChanged,
    PlanCreated,
    MilestoneCreated,
    ContributorAdded,
    ContributionRecorded,
    ContributionStatusChanged,
    PayoutRequested,
    PayoutApproved,
    PayoutRejected,
    PayoutStatusChanged,
    SchoolVerified,
    ScholarshipDecision,
    AchievementCredentialIssued,
    BlockchainReferenceAttached,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserRegistered => "user.registered",
            Self::KycStatusChanged => "kyc.status_changed",
            Self::PlanCreated => "savings_plan.created",
            Self::MilestoneCreated => "milestone.created",
            Self::ContributorAdded => "vault_contributor.added",
            Self::ContributionRecorded => "contribution.recorded",
            Self::ContributionStatusChanged => "contribution.status_changed",
            Self::PayoutRequested => "payout.requested",
            Self::PayoutApproved => "payout.approved",
            Self::PayoutRejected => "payout.rejected",
            Self::PayoutStatusChanged => "payout.status_changed",
            Self::SchoolVerified => "school.verified",
            Self::ScholarshipDecision => "scholarship.decision",
            Self::AchievementCredentialIssued => "achievement_credential.issued",
            Self::BlockchainReferenceAttached => "blockchain.reference_attached",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub actor_user_id: Option<Uuid>,
    pub entity_type: AuditEntityType,
    pub entity_id: Option<Uuid>,
    pub action: AuditAction,
    pub metadata: Value,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AuditContext {
    pub request_id: String,
    pub correlation_id: String,
}

tokio::task_local! {
    static CURRENT_AUDIT_CONTEXT: AuditContext;
}

impl AuditEvent {
    pub fn new(
        actor_user_id: Option<Uuid>,
        entity_type: AuditEntityType,
        entity_id: Option<Uuid>,
        action: AuditAction,
        metadata: Value,
    ) -> Self {
        Self { actor_user_id, entity_type, entity_id, action, metadata, occurred_at: Utc::now() }
    }

    pub fn user_registered(user_id: Uuid, role: &str, email: &str) -> Self {
        Self::new(
            Some(user_id),
            AuditEntityType::User,
            Some(user_id),
            AuditAction::UserRegistered,
            json!({
                "role": role,
                "email_domain": email.split('@').nth(1).unwrap_or_default(),
            }),
        )
    }

    pub fn kyc_status_changed(
        actor_user_id: Option<Uuid>,
        profile: &KycProfile,
        previous_status: Option<&str>,
    ) -> Self {
        Self::new(
            actor_user_id,
            AuditEntityType::KycProfile,
            Some(profile.id),
            AuditAction::KycStatusChanged,
            json!({
                "user_id": profile.user_id,
                "from_status": previous_status,
                "to_status": profile.status.as_str(),
            }),
        )
    }

    pub fn plan_created(actor_user_id: Uuid, plan: &SavingsPlan) -> Self {
        Self::new(
            Some(actor_user_id),
            AuditEntityType::SavingsPlan,
            Some(plan.id),
            AuditAction::PlanCreated,
            json!({
                "child_profile_id": plan.child_profile_id,
                "owner_user_id": plan.owner_user_id,
                "target_amount_minor": plan.target_amount.amount_minor,
                "currency": plan.target_amount.currency.as_str(),
                "status": plan.status.as_str(),
            }),
        )
    }

    pub fn milestone_created(actor_user_id: Uuid, milestone: &Milestone) -> Self {
        Self::new(
            Some(actor_user_id),
            AuditEntityType::Milestone,
            Some(milestone.id),
            AuditAction::MilestoneCreated,
            json!({
                "vault_id": milestone.vault_id,
                "due_date": milestone.due_date,
                "target_amount_minor": milestone.target_amount.amount_minor,
                "currency": milestone.target_amount.currency.as_str(),
                "payout_type": milestone.payout_type.as_str(),
            }),
        )
    }

    pub fn contributor_added(actor_user_id: Uuid, contributor: &VaultContributor) -> Self {
        Self::new(
            Some(actor_user_id),
            AuditEntityType::VaultContributor,
            Some(contributor.id),
            AuditAction::ContributorAdded,
            json!({
                "vault_id": contributor.vault_id,
                "contributor_user_id": contributor.contributor_user_id,
                "role_label": contributor.role_label,
            }),
        )
    }

    pub fn contribution_recorded(actor_user_id: Uuid, contribution: &Contribution) -> Self {
        Self::new(
            Some(actor_user_id),
            AuditEntityType::Contribution,
            Some(contribution.id),
            AuditAction::ContributionRecorded,
            json!({
                "vault_id": contribution.vault_id,
                "contributor_user_id": contribution.contributor_user_id,
                "amount_minor": contribution.amount.amount_minor,
                "currency": contribution.amount.currency.as_str(),
                "source_type": contribution.source_type.as_str(),
                "status": contribution.status.as_str(),
            }),
        )
    }

    pub fn contribution_status_changed(
        actor_user_id: Uuid,
        contribution: &Contribution,
        to_status: &str,
        external_reference_present: bool,
    ) -> Self {
        Self::new(
            Some(actor_user_id),
            AuditEntityType::Contribution,
            Some(contribution.id),
            AuditAction::ContributionStatusChanged,
            json!({
                "vault_id": contribution.vault_id,
                "from_status": contribution.status.as_str(),
                "to_status": to_status,
                "external_reference_present": external_reference_present,
            }),
        )
    }

    pub fn payout_requested(actor_user_id: Uuid, payout: &PayoutRequest) -> Self {
        Self::new(
            Some(actor_user_id),
            AuditEntityType::PayoutRequest,
            Some(payout.id),
            AuditAction::PayoutRequested,
            json!({
                "vault_id": payout.vault_id,
                "milestone_id": payout.milestone_id,
                "school_id": payout.school_id,
                "amount_minor": payout.amount.amount_minor,
                "currency": payout.amount.currency.as_str(),
                "status": payout.status.as_str(),
            }),
        )
    }

    pub fn payout_decision(actor_user_id: Uuid, payout: &PayoutRequest, to_status: &str) -> Self {
        let action = match to_status {
            "approved" => AuditAction::PayoutApproved,
            "rejected" => AuditAction::PayoutRejected,
            _ => AuditAction::PayoutStatusChanged,
        };

        Self::new(
            Some(actor_user_id),
            AuditEntityType::PayoutRequest,
            Some(payout.id),
            action,
            json!({
                "vault_id": payout.vault_id,
                "milestone_id": payout.milestone_id,
                "school_id": payout.school_id,
                "from_status": payout.status.as_str(),
                "to_status": to_status,
                "review_notes_present": payout.review_notes.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
                "external_payout_reference_present": payout.external_payout_reference.is_some(),
            }),
        )
    }

    pub fn school_verified(actor_user_id: Uuid, school: &School) -> Self {
        Self::new(
            Some(actor_user_id),
            AuditEntityType::School,
            Some(school.id),
            AuditAction::SchoolVerified,
            json!({
                "verification_status": school.verification_status.as_str(),
                "country": school.country,
                "payout_method": school.payout_method.as_str(),
            }),
        )
    }

    pub fn scholarship_decision(
        actor_user_id: Uuid,
        application_id: Uuid,
        award: &ScholarshipAward,
        pool: &ScholarshipPool,
    ) -> Self {
        Self::new(
            Some(actor_user_id),
            AuditEntityType::ScholarshipAward,
            Some(award.id),
            AuditAction::ScholarshipDecision,
            json!({
                "application_id": application_id,
                "pool_id": pool.id,
                "decision": award.status.as_str(),
                "amount_minor": award.amount.amount_minor,
                "currency": award.amount.currency.as_str(),
            }),
        )
    }

    pub fn blockchain_reference_attached(
        actor_user_id: Option<Uuid>,
        reference: &ExternalReference,
    ) -> Self {
        Self::new(
            actor_user_id,
            AuditEntityType::ExternalReference,
            Some(reference.id),
            AuditAction::BlockchainReferenceAttached,
            json!({
                "entity_type": reference.entity_type.as_str(),
                "entity_id": reference.entity_id,
                "reference_kind": reference.reference_kind.as_str(),
                "reference_preview": preview_value(&reference.value),
            }),
        )
    }

    pub fn achievement_credential_issued(
        actor_user_id: Uuid,
        credential: &AchievementCredential,
    ) -> Self {
        Self::new(
            Some(actor_user_id),
            AuditEntityType::AchievementCredential,
            Some(credential.id),
            AuditAction::AchievementCredentialIssued,
            json!({
                "credential_ref": credential.credential_ref,
                "child_profile_id": credential.child_profile_id,
                "recipient_user_id": credential.recipient_user_id,
                "school_id": credential.school_id,
                "achievement_type": credential.achievement_type.as_str(),
                "status": credential.status.as_str(),
                "achievement_date": credential.achievement_date,
                "issued_by_role": credential.issued_by_role,
                "attestation_method": credential.attestation_method,
                "attestation_anchor_present": credential.attestation_anchor.is_some(),
                "evidence_uri_present": credential.evidence_uri.is_some(),
            }),
        )
    }

    pub fn into_log(self) -> AuditLog {
        AuditLog {
            id: Uuid::new_v4(),
            actor_user_id: self.actor_user_id,
            entity_type: self.entity_type.as_str().to_owned(),
            entity_id: self.entity_id,
            action: self.action.as_str().to_owned(),
            request_id: None,
            correlation_id: None,
            metadata: self.metadata,
            created_at: self.occurred_at,
            updated_at: self.occurred_at,
        }
    }
}

impl AuditContext {
    pub fn attach_to_log(&self, mut audit_log: AuditLog) -> AuditLog {
        audit_log.request_id = Some(self.request_id.clone());
        audit_log.correlation_id = Some(self.correlation_id.clone());
        audit_log
    }
}

pub async fn scope_audit_context<F, T>(context: AuditContext, future: F) -> T
where
    F: Future<Output = T>,
{
    CURRENT_AUDIT_CONTEXT.scope(context, future).await
}

pub fn current_audit_context() -> Option<AuditContext> {
    CURRENT_AUDIT_CONTEXT.try_with(Clone::clone).ok()
}

pub struct AuditService<'a, R> {
    repo: &'a R,
}

impl<'a, R> AuditService<'a, R>
where
    R: AuditLogRepository,
{
    pub fn new(repo: &'a R) -> Self {
        Self { repo }
    }

    pub async fn record(&self, event: AuditEvent) -> Result<AuditLog, AuditError> {
        self.repo.append(event.into_log()).await.map_err(Into::into)
    }

    pub async fn list_by_entity(
        &self,
        entity_type: AuditEntityType,
        entity_id: Uuid,
    ) -> Result<Vec<AuditLog>, AuditError> {
        self.repo.list_by_entity(entity_type.as_str(), entity_id).await.map_err(Into::into)
    }

    pub async fn list_by_actor(&self, actor_user_id: Uuid) -> Result<Vec<AuditLog>, AuditError> {
        self.repo.list_by_actor(actor_user_id).await.map_err(Into::into)
    }
}

fn preview_value(value: &str) -> String {
    if value.len() <= 12 {
        return value.to_owned();
    }

    format!("{}...{}", &value[..6], &value[value.len() - 4..])
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use super::*;

    #[derive(Default, Clone)]
    struct FakeAuditRepo {
        items: Arc<Mutex<Vec<AuditLog>>>,
        by_entity: Arc<Mutex<HashMap<(String, Uuid), Vec<AuditLog>>>>,
    }

    #[async_trait]
    impl AuditLogRepository for FakeAuditRepo {
        async fn append(&self, audit_log: AuditLog) -> Result<AuditLog, PersistenceError> {
            self.items.lock().await.push(audit_log.clone());
            if let Some(entity_id) = audit_log.entity_id {
                self.by_entity
                    .lock()
                    .await
                    .entry((audit_log.entity_type.clone(), entity_id))
                    .or_default()
                    .push(audit_log.clone());
            }
            Ok(audit_log)
        }

        async fn list_by_entity(
            &self,
            entity_type: &str,
            entity_id: Uuid,
        ) -> Result<Vec<AuditLog>, PersistenceError> {
            Ok(self
                .by_entity
                .lock()
                .await
                .get(&(entity_type.to_owned(), entity_id))
                .cloned()
                .unwrap_or_default())
        }

        async fn list_by_actor(
            &self,
            actor_user_id: Uuid,
        ) -> Result<Vec<AuditLog>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .await
                .iter()
                .filter(|item| item.actor_user_id == Some(actor_user_id))
                .cloned()
                .collect())
        }
    }

    #[tokio::test]
    async fn service_records_and_queries_standardized_events() {
        let repo = FakeAuditRepo::default();
        let service = AuditService::new(&repo);
        let user_id = Uuid::new_v4();

        let saved = service
            .record(AuditEvent::user_registered(user_id, "parent", "person@example.com"))
            .await
            .unwrap();

        assert_eq!(saved.entity_type, "user");
        assert_eq!(saved.action, "user.registered");
        assert_eq!(saved.metadata["email_domain"], "example.com");

        let by_entity = service.list_by_entity(AuditEntityType::User, user_id).await.unwrap();
        let by_actor = service.list_by_actor(user_id).await.unwrap();

        assert_eq!(by_entity.len(), 1);
        assert_eq!(by_actor.len(), 1);
    }
}
