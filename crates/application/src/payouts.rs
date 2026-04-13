use chrono::Utc;
use domain::{
    auth::AuthenticatedUser,
    payouts::{
        PayoutRuleViolation, PayoutTransition, ensure_available_funds, validate_currency_match,
        validate_milestone_association, validate_milestone_payable, validate_payout_amount,
        validate_requester_role, validate_reviewer_role, validate_school_verified,
        validate_transition,
    },
    persistence::PayoutRequest,
};
use serde_json::json;
use shared::{currency::Currency, money::Money};
use uuid::Uuid;

use crate::audit::AuditEvent;
use crate::repos::{
    MilestoneRepository, PayoutRepository, PayoutWorkflowRepository, PersistenceError,
    SchoolRepository, VaultRepository,
};

#[derive(Debug, thiserror::Error)]
pub enum PayoutError {
    #[error("{0}")]
    Validation(String),
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("repository error: {0}")]
    Repository(String),
}

impl From<PersistenceError> for PayoutError {
    fn from(value: PersistenceError) -> Self {
        match value {
            PersistenceError::NotFound => Self::NotFound,
            PersistenceError::Conflict(message) => Self::Conflict(message),
            PersistenceError::Validation(message) => Self::Validation(message),
            PersistenceError::Repository(message) => Self::Repository(message),
        }
    }
}

impl From<PayoutRuleViolation> for PayoutError {
    fn from(value: PayoutRuleViolation) -> Self {
        match value {
            PayoutRuleViolation::UnauthorizedRequester
            | PayoutRuleViolation::UnauthorizedReviewer => Self::Forbidden,
            PayoutRuleViolation::InvalidStatusTransition { .. }
            | PayoutRuleViolation::InsufficientAvailableFunds
            | PayoutRuleViolation::SchoolNotVerified => Self::Conflict(value.to_string()),
            _ => Self::Validation(value.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreatePayoutRequestInput {
    pub vault_id: Uuid,
    pub milestone_id: Uuid,
    pub school_id: Uuid,
    pub amount_minor: i64,
    pub currency: Currency,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct ReviewPayoutInput {
    pub payout_id: Uuid,
    pub review_notes: Option<String>,
    pub external_payout_reference: Option<String>,
}

pub async fn create_payout_request<V, M, S, R, W>(
    vaults: &V,
    milestones: &M,
    schools: &S,
    payouts: &R,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: CreatePayoutRequestInput,
) -> Result<PayoutRequest, PayoutError>
where
    V: VaultRepository,
    M: MilestoneRepository,
    S: SchoolRepository,
    R: PayoutRepository,
    W: PayoutWorkflowRepository,
{
    validate_requester_role(actor.role)?;
    validate_payout_amount(input.amount_minor)?;

    if let Some(existing) = payouts.find_by_idempotency_key(&input.idempotency_key).await? {
        if existing.vault_id == input.vault_id
            && existing.milestone_id == input.milestone_id
            && existing.school_id == input.school_id
            && existing.requested_by == actor.user_id
            && existing.amount.amount_minor == input.amount_minor
            && existing.amount.currency == input.currency
        {
            return Ok(existing);
        }

        return Err(PayoutError::Conflict(
            "idempotency key already used for a different payout request".to_owned(),
        ));
    }

    let vault = vaults.find_by_id(input.vault_id).await?.ok_or(PayoutError::NotFound)?;
    if vault.owner_user_id != actor.user_id {
        return Err(PayoutError::Forbidden);
    }

    let milestone =
        milestones.find_by_id(input.milestone_id).await?.ok_or(PayoutError::NotFound)?;
    let school = schools.find_by_id(input.school_id).await?.ok_or(PayoutError::NotFound)?;

    validate_milestone_association(milestone.vault_id, input.vault_id)?;
    validate_milestone_payable(milestone.status)?;
    validate_currency_match(input.currency.as_str(), &vault.currency)?;
    validate_school_verified(school.verification_status)?;
    ensure_available_funds(
        vault.total_contributed_minor,
        vault.total_locked_minor,
        vault.total_disbursed_minor,
        input.amount_minor,
    )?;

    let now = Utc::now();
    let payout = PayoutRequest {
        id: Uuid::new_v4(),
        vault_id: input.vault_id,
        milestone_id: input.milestone_id,
        school_id: input.school_id,
        requested_by: actor.user_id,
        amount: Money::new(input.amount_minor, input.currency)
            .map_err(|message| PayoutError::Validation(message.to_owned()))?,
        idempotency_key: Some(input.idempotency_key.clone()),
        status: domain::persistence::PayoutStatus::Pending,
        review_notes: None,
        external_payout_reference: None,
        reviewed_by: None,
        reviewed_at: None,
        created_at: now,
        updated_at: now,
    };
    let mut audit = AuditEvent::payout_requested(actor.user_id, &payout).into_log();
    audit.metadata = json!({
        "vault_id": vault.id,
        "milestone_id": milestone.id,
        "school_id": school.id,
        "school_verification_status": school.verification_status.as_str(),
        "amount_minor": payout.amount.amount_minor,
        "currency": payout.amount.currency.as_str(),
        "status": payout.status.as_str(),
    });

    workflow.create_request_with_audit(payout, audit).await.map_err(Into::into)
}

pub async fn get_payout<R>(
    payouts: &R,
    actor: &AuthenticatedUser,
    payout_id: Uuid,
) -> Result<PayoutRequest, PayoutError>
where
    R: PayoutRepository,
{
    let payout = payouts.find_by_id(payout_id).await?.ok_or(PayoutError::NotFound)?;

    if actor.user_id != payout.requested_by && actor.role != domain::auth::UserRole::PlatformAdmin {
        return Err(PayoutError::Forbidden);
    }

    Ok(payout)
}

pub async fn list_vault_payouts<V, R>(
    vaults: &V,
    payouts: &R,
    actor: &AuthenticatedUser,
    vault_id: Uuid,
) -> Result<Vec<PayoutRequest>, PayoutError>
where
    V: VaultRepository,
    R: PayoutRepository,
{
    let vault = vaults.find_by_id(vault_id).await?.ok_or(PayoutError::NotFound)?;

    if actor.user_id != vault.owner_user_id && actor.role != domain::auth::UserRole::PlatformAdmin {
        return Err(PayoutError::Forbidden);
    }

    payouts.list_by_vault(vault_id).await.map_err(Into::into)
}

pub async fn move_payout_to_review<R, S, M, V, W>(
    payouts: &R,
    schools: &S,
    milestones: &M,
    vaults: &V,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: ReviewPayoutInput,
) -> Result<PayoutRequest, PayoutError>
where
    R: PayoutRepository,
    S: SchoolRepository,
    M: MilestoneRepository,
    V: VaultRepository,
    W: PayoutWorkflowRepository,
{
    review_transition(
        payouts,
        schools,
        milestones,
        vaults,
        workflow,
        actor,
        input,
        PayoutTransition::MoveToReview,
        "payout.review_started",
    )
    .await
}

pub async fn approve_payout<R, S, M, V, W>(
    payouts: &R,
    schools: &S,
    milestones: &M,
    vaults: &V,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: ReviewPayoutInput,
) -> Result<PayoutRequest, PayoutError>
where
    R: PayoutRepository,
    S: SchoolRepository,
    M: MilestoneRepository,
    V: VaultRepository,
    W: PayoutWorkflowRepository,
{
    review_transition(
        payouts,
        schools,
        milestones,
        vaults,
        workflow,
        actor,
        input,
        PayoutTransition::Approve,
        "payout.approved",
    )
    .await
}

pub async fn reject_payout<R, S, M, V, W>(
    payouts: &R,
    schools: &S,
    milestones: &M,
    vaults: &V,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: ReviewPayoutInput,
) -> Result<PayoutRequest, PayoutError>
where
    R: PayoutRepository,
    S: SchoolRepository,
    M: MilestoneRepository,
    V: VaultRepository,
    W: PayoutWorkflowRepository,
{
    review_transition(
        payouts,
        schools,
        milestones,
        vaults,
        workflow,
        actor,
        input,
        PayoutTransition::Reject,
        "payout.rejected",
    )
    .await
}

pub async fn mark_payout_processing<R, S, M, V, W>(
    payouts: &R,
    schools: &S,
    milestones: &M,
    vaults: &V,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: ReviewPayoutInput,
) -> Result<PayoutRequest, PayoutError>
where
    R: PayoutRepository,
    S: SchoolRepository,
    M: MilestoneRepository,
    V: VaultRepository,
    W: PayoutWorkflowRepository,
{
    review_transition(
        payouts,
        schools,
        milestones,
        vaults,
        workflow,
        actor,
        input,
        PayoutTransition::MarkProcessing,
        "payout.processing_started",
    )
    .await
}

pub async fn complete_payout<R, S, M, V, W>(
    payouts: &R,
    schools: &S,
    milestones: &M,
    vaults: &V,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: ReviewPayoutInput,
) -> Result<PayoutRequest, PayoutError>
where
    R: PayoutRepository,
    S: SchoolRepository,
    M: MilestoneRepository,
    V: VaultRepository,
    W: PayoutWorkflowRepository,
{
    review_transition(
        payouts,
        schools,
        milestones,
        vaults,
        workflow,
        actor,
        input,
        PayoutTransition::Complete,
        "payout.completed",
    )
    .await
}

pub async fn fail_payout<R, S, M, V, W>(
    payouts: &R,
    schools: &S,
    milestones: &M,
    vaults: &V,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: ReviewPayoutInput,
) -> Result<PayoutRequest, PayoutError>
where
    R: PayoutRepository,
    S: SchoolRepository,
    M: MilestoneRepository,
    V: VaultRepository,
    W: PayoutWorkflowRepository,
{
    review_transition(
        payouts,
        schools,
        milestones,
        vaults,
        workflow,
        actor,
        input,
        PayoutTransition::Fail,
        "payout.failed",
    )
    .await
}

async fn review_transition<R, S, M, V, W>(
    payouts: &R,
    schools: &S,
    milestones: &M,
    vaults: &V,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: ReviewPayoutInput,
    transition: PayoutTransition,
    audit_action: &'static str,
) -> Result<PayoutRequest, PayoutError>
where
    R: PayoutRepository,
    S: SchoolRepository,
    M: MilestoneRepository,
    V: VaultRepository,
    W: PayoutWorkflowRepository,
{
    validate_reviewer_role(actor.role)?;

    let payout = payouts.find_by_id(input.payout_id).await?.ok_or(PayoutError::NotFound)?;
    let target_status = validate_transition(payout.status, transition)?;
    if payout.status == target_status {
        return Ok(payout);
    }

    let school = schools.find_by_id(payout.school_id).await?.ok_or(PayoutError::NotFound)?;
    let milestone =
        milestones.find_by_id(payout.milestone_id).await?.ok_or(PayoutError::NotFound)?;
    let vault = vaults.find_by_id(payout.vault_id).await?.ok_or(PayoutError::NotFound)?;

    validate_milestone_association(milestone.vault_id, payout.vault_id)?;
    validate_milestone_payable(milestone.status)?;
    if matches!(
        transition,
        PayoutTransition::Approve
            | PayoutTransition::MarkProcessing
            | PayoutTransition::Complete
            | PayoutTransition::Fail
    ) {
        validate_school_verified(school.verification_status)?;
    }
    if matches!(transition, PayoutTransition::Approve) {
        ensure_available_funds(
            vault.total_contributed_minor,
            vault.total_locked_minor,
            vault.total_disbursed_minor,
            payout.amount.amount_minor,
        )?;
    }

    let updated_preview = PayoutRequest {
        status: target_status,
        review_notes: input.review_notes.clone(),
        external_payout_reference: input.external_payout_reference.clone(),
        ..payout.clone()
    };
    let mut audit =
        AuditEvent::payout_decision(actor.user_id, &updated_preview, target_status.as_str())
            .into_log();
    audit.action = audit_action.to_owned();
    audit.metadata = json!({
        "vault_id": vault.id,
        "milestone_id": milestone.id,
        "school_id": school.id,
        "school_verification_status": school.verification_status.as_str(),
        "from_status": payout.status.as_str(),
        "to_status": target_status.as_str(),
        "review_notes_present": input.review_notes.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false),
        "external_payout_reference_present": input.external_payout_reference.is_some(),
    });

    let result = workflow
        .transition_with_audit(
            payout.id,
            target_status.as_str(),
            actor.user_id,
            input.review_notes.as_deref(),
            input.external_payout_reference.as_deref(),
            audit,
        )
        .await?;

    Ok(result.payout)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::{NaiveDate, Utc};
    use domain::{
        auth::{AuthenticatedUser, UserRole},
        persistence::{
            AuditLog, Milestone, MilestoneStatus, PayoutRequest, PayoutStatus, PayoutType,
            SavingsVault, School, SchoolPayoutMethod, SchoolVerificationStatus, VaultContributor,
            VaultStatus,
        },
    };
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use crate::repos::{
        MilestoneRepository, PayoutRepository, PayoutTransitionResult, PayoutWorkflowRepository,
        PersistenceError, SchoolRepository, VaultRepository,
    };

    use super::{
        CreatePayoutRequestInput, PayoutError, ReviewPayoutInput, approve_payout,
        create_payout_request, move_payout_to_review,
    };

    #[derive(Default)]
    struct State {
        vaults: HashMap<Uuid, SavingsVault>,
        milestones: HashMap<Uuid, Milestone>,
        schools: HashMap<Uuid, School>,
        payouts: HashMap<Uuid, PayoutRequest>,
        audits: Vec<AuditLog>,
    }

    #[derive(Clone, Default)]
    struct Repos {
        state: Arc<Mutex<State>>,
    }

    #[async_trait]
    impl VaultRepository for Repos {
        async fn create(&self, _: SavingsVault) -> Result<SavingsVault, PersistenceError> {
            unimplemented!()
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<SavingsVault>, PersistenceError> {
            Ok(self.state.lock().await.vaults.get(&id).cloned())
        }
        async fn update_balances(
            &self,
            _: Uuid,
            _: i64,
            _: i64,
            _: i64,
        ) -> Result<(), PersistenceError> {
            unimplemented!()
        }
        async fn add_contributor(
            &self,
            _: VaultContributor,
        ) -> Result<VaultContributor, PersistenceError> {
            unimplemented!()
        }
        async fn list_contributors(
            &self,
            _: Uuid,
        ) -> Result<Vec<VaultContributor>, PersistenceError> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl MilestoneRepository for Repos {
        async fn create(&self, _: Milestone) -> Result<Milestone, PersistenceError> {
            unimplemented!()
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<Milestone>, PersistenceError> {
            Ok(self.state.lock().await.milestones.get(&id).cloned())
        }
        async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<Milestone>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .milestones
                .values()
                .filter(|item| item.vault_id == vault_id)
                .cloned()
                .collect())
        }
        async fn update_funded_amount(
            &self,
            _: Uuid,
            _: i64,
            _: &str,
        ) -> Result<(), PersistenceError> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl SchoolRepository for Repos {
        async fn create(&self, _: School) -> Result<School, PersistenceError> {
            unimplemented!()
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<School>, PersistenceError> {
            Ok(self.state.lock().await.schools.get(&id).cloned())
        }
        async fn search_verified(&self, _: Option<&str>) -> Result<Vec<School>, PersistenceError> {
            Ok(vec![])
        }
        async fn list_verified(&self) -> Result<Vec<School>, PersistenceError> {
            Ok(vec![])
        }
        async fn update_verification(
            &self,
            _: Uuid,
            _: &str,
            _: Option<Uuid>,
            _: Option<chrono::DateTime<chrono::Utc>>,
        ) -> Result<(), PersistenceError> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl PayoutRepository for Repos {
        async fn create(&self, _: PayoutRequest) -> Result<PayoutRequest, PersistenceError> {
            unimplemented!()
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<PayoutRequest>, PersistenceError> {
            Ok(self.state.lock().await.payouts.get(&id).cloned())
        }
        async fn find_by_idempotency_key(
            &self,
            idempotency_key: &str,
        ) -> Result<Option<PayoutRequest>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .payouts
                .values()
                .find(|item| item.idempotency_key.as_deref() == Some(idempotency_key))
                .cloned())
        }
        async fn list_by_vault(
            &self,
            vault_id: Uuid,
        ) -> Result<Vec<PayoutRequest>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .payouts
                .values()
                .filter(|item| item.vault_id == vault_id)
                .cloned()
                .collect())
        }
        async fn update_status(
            &self,
            _: Uuid,
            _: &str,
            _: Option<&str>,
            _: Option<&str>,
        ) -> Result<(), PersistenceError> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl PayoutWorkflowRepository for Repos {
        async fn create_request_with_audit(
            &self,
            payout: PayoutRequest,
            audit_log: AuditLog,
        ) -> Result<PayoutRequest, PersistenceError> {
            let mut state = self.state.lock().await;
            state.payouts.insert(payout.id, payout.clone());
            state.audits.push(audit_log);
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
            let mut state = self.state.lock().await;
            let mut payout =
                state.payouts.get(&payout_id).cloned().ok_or(PersistenceError::NotFound)?;
            let mut vault =
                state.vaults.get(&payout.vault_id).cloned().ok_or(PersistenceError::NotFound)?;
            payout.status = match status {
                "under_review" => PayoutStatus::UnderReview,
                "approved" => PayoutStatus::Approved,
                "rejected" => PayoutStatus::Rejected,
                "processing" => PayoutStatus::Processing,
                "completed" => PayoutStatus::Completed,
                "failed" => PayoutStatus::Failed,
                _ => return Err(PersistenceError::Validation("invalid status".to_owned())),
            };
            payout.review_notes =
                review_notes.map(ToOwned::to_owned).or_else(|| payout.review_notes.clone());
            payout.external_payout_reference = external_payout_reference
                .map(ToOwned::to_owned)
                .or_else(|| payout.external_payout_reference.clone());
            payout.reviewed_by = Some(actor_user_id);
            payout.reviewed_at = Some(Utc::now());

            match payout.status {
                PayoutStatus::Approved => vault.total_locked_minor += payout.amount.amount_minor,
                PayoutStatus::Rejected => {}
                PayoutStatus::Processing => {}
                PayoutStatus::Completed => {
                    vault.total_locked_minor -= payout.amount.amount_minor;
                    vault.total_disbursed_minor += payout.amount.amount_minor;
                }
                PayoutStatus::Failed => {
                    if vault.total_locked_minor >= payout.amount.amount_minor {
                        vault.total_locked_minor -= payout.amount.amount_minor;
                    }
                }
                PayoutStatus::Pending | PayoutStatus::UnderReview => {}
            }

            state.payouts.insert(payout.id, payout.clone());
            state.vaults.insert(vault.id, vault.clone());
            state.audits.push(audit_log);

            Ok(PayoutTransitionResult { payout, vault })
        }
    }

    #[tokio::test]
    async fn cannot_request_payout_for_unverified_school() {
        let repos = seeded_repos(true).await;
        let error = create_payout_request(
            &repos,
            &repos,
            &repos,
            &repos,
            &repos,
            &parent(),
            CreatePayoutRequestInput {
                vault_id: seeded_vault_id(),
                milestone_id: seeded_milestone_id(),
                school_id: seeded_school_id(),
                amount_minor: 30_000,
                currency: shared::currency::Currency::Usdc,
                idempotency_key: "payout-unverified-school".to_owned(),
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, PayoutError::Conflict(_)));
    }

    #[tokio::test]
    async fn cannot_approve_payout_for_unverified_school() {
        let repos = seeded_repos(false).await;
        let payout = seed_payout(&repos).await;
        {
            let mut state = repos.state.lock().await;
            state.schools.get_mut(&seeded_school_id()).unwrap().verification_status =
                SchoolVerificationStatus::Rejected;
            state.payouts.get_mut(&payout.id).unwrap().status = PayoutStatus::UnderReview;
        }

        let error = approve_payout(
            &repos,
            &repos,
            &repos,
            &repos,
            &repos,
            &platform_admin(),
            ReviewPayoutInput {
                payout_id: payout.id,
                review_notes: Some("nope".to_owned()),
                external_payout_reference: None,
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, PayoutError::Conflict(_)));
    }

    #[tokio::test]
    async fn approval_requires_review_state() {
        let repos = seeded_repos(false).await;
        let payout = seed_payout(&repos).await;

        let error = approve_payout(
            &repos,
            &repos,
            &repos,
            &repos,
            &repos,
            &platform_admin(),
            ReviewPayoutInput {
                payout_id: payout.id,
                review_notes: None,
                external_payout_reference: None,
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, PayoutError::Conflict(_)));
    }

    #[tokio::test]
    async fn cannot_request_payout_when_milestone_belongs_to_another_vault() {
        let repos = seeded_repos(false).await;
        {
            let mut state = repos.state.lock().await;
            state.milestones.get_mut(&seeded_milestone_id()).unwrap().vault_id = Uuid::new_v4();
        }

        let error = create_payout_request(
            &repos,
            &repos,
            &repos,
            &repos,
            &repos,
            &parent(),
            CreatePayoutRequestInput {
                vault_id: seeded_vault_id(),
                milestone_id: seeded_milestone_id(),
                school_id: seeded_school_id(),
                amount_minor: 30_000,
                currency: shared::currency::Currency::Usdc,
                idempotency_key: "payout-other-vault".to_owned(),
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, PayoutError::Validation(_)));
    }

    #[tokio::test]
    async fn approve_locks_funds_after_review() {
        let repos = seeded_repos(false).await;
        let payout = seed_payout(&repos).await;

        let review = move_payout_to_review(
            &repos,
            &repos,
            &repos,
            &repos,
            &repos,
            &platform_admin(),
            ReviewPayoutInput {
                payout_id: payout.id,
                review_notes: Some("reviewing".to_owned()),
                external_payout_reference: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(review.status, PayoutStatus::UnderReview);

        let approved = approve_payout(
            &repos,
            &repos,
            &repos,
            &repos,
            &repos,
            &platform_admin(),
            ReviewPayoutInput {
                payout_id: payout.id,
                review_notes: Some("approved".to_owned()),
                external_payout_reference: Some("offramp:123".to_owned()),
            },
        )
        .await
        .unwrap();

        let state = repos.state.lock().await;
        assert_eq!(approved.status, PayoutStatus::Approved);
        assert_eq!(state.vaults.get(&seeded_vault_id()).unwrap().total_locked_minor, 30_000);
        assert_eq!(state.audits.len(), 3);
    }

    async fn seeded_repos(unverified_school: bool) -> Repos {
        let repos = Repos::default();
        let mut state = repos.state.lock().await;
        state.vaults.insert(
            seeded_vault_id(),
            SavingsVault {
                id: seeded_vault_id(),
                plan_id: Uuid::new_v4(),
                owner_user_id: parent().user_id,
                currency: "USDC".to_owned(),
                status: VaultStatus::Active,
                total_contributed_minor: 100_000,
                total_locked_minor: 0,
                total_disbursed_minor: 0,
                external_wallet_account_id: None,
                external_contract_ref: None,
                version: 0,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        state.milestones.insert(
            seeded_milestone_id(),
            Milestone {
                id: seeded_milestone_id(),
                vault_id: seeded_vault_id(),
                title: "Tuition".to_owned(),
                description: None,
                due_date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
                target_amount: shared::money::Money::new(50_000, shared::currency::Currency::Usdc)
                    .unwrap(),
                funded_amount: shared::money::Money::new(50_000, shared::currency::Currency::Usdc)
                    .unwrap(),
                payout_type: PayoutType::Tuition,
                status: MilestoneStatus::Funded,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        state.schools.insert(
            seeded_school_id(),
            School {
                id: seeded_school_id(),
                legal_name: "Legal School Ltd".to_owned(),
                display_name: "Bright Future Academy".to_owned(),
                country: "NG".to_owned(),
                payout_method: SchoolPayoutMethod::FiatOfframp,
                payout_reference: "bank:001:12345".to_owned(),
                verification_status: if unverified_school {
                    SchoolVerificationStatus::Pending
                } else {
                    SchoolVerificationStatus::Verified
                },
                verified_by: if unverified_school { None } else { Some(platform_admin().user_id) },
                verified_at: if unverified_school { None } else { Some(Utc::now()) },
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        drop(state);
        repos
    }

    async fn seed_payout(repos: &Repos) -> PayoutRequest {
        create_payout_request(
            repos,
            repos,
            repos,
            repos,
            repos,
            &parent(),
            CreatePayoutRequestInput {
                vault_id: seeded_vault_id(),
                milestone_id: seeded_milestone_id(),
                school_id: seeded_school_id(),
                amount_minor: 30_000,
                currency: shared::currency::Currency::Usdc,
                idempotency_key: "payout-seeded".to_owned(),
            },
        )
        .await
        .unwrap()
    }

    fn parent() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::from_u128(11),
            role: UserRole::Parent,
            session_id: Uuid::new_v4(),
        }
    }

    fn platform_admin() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::from_u128(12),
            role: UserRole::PlatformAdmin,
            session_id: Uuid::new_v4(),
        }
    }

    fn seeded_vault_id() -> Uuid {
        Uuid::from_u128(101)
    }

    fn seeded_milestone_id() -> Uuid {
        Uuid::from_u128(102)
    }

    fn seeded_school_id() -> Uuid {
        Uuid::from_u128(103)
    }
}
