use chrono::Utc;
use domain::{
    auth::{AuthenticatedUser, UserRole},
    persistence::{
        AuditLog, DonorContribution, DonorContributionStatus, ScholarshipApplication,
        ScholarshipApplicationStatus, ScholarshipAward, ScholarshipAwardStatus, ScholarshipPool,
        ScholarshipPoolStatus,
    },
    scholarships::{
        AwardTransition, ScholarshipRuleViolation, can_apply, can_fund_pool, can_review,
        ensure_pool_balance, validate_application_reviewable, validate_award_transition,
        validate_child_profile_access, validate_currency, validate_pool_is_open,
        validate_pool_restrictions, validate_positive_amount,
    },
};
use serde_json::json;
use shared::{currency::Currency, money::Money};
use uuid::Uuid;

use crate::audit::AuditEvent;
use crate::repos::{
    ChildProfileRepository, DonorContributionRepository, PersistenceError,
    ScholarshipApplicationRepository, ScholarshipAwardRepository, ScholarshipPoolRepository,
    ScholarshipWorkflowRepository,
};

#[derive(Debug, thiserror::Error)]
pub enum ScholarshipError {
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

impl From<PersistenceError> for ScholarshipError {
    fn from(value: PersistenceError) -> Self {
        match value {
            PersistenceError::NotFound => Self::NotFound,
            PersistenceError::Conflict(message) => Self::Conflict(message),
            PersistenceError::Validation(message) => Self::Validation(message),
            PersistenceError::Repository(message) => Self::Repository(message),
        }
    }
}

impl From<ScholarshipRuleViolation> for ScholarshipError {
    fn from(value: ScholarshipRuleViolation) -> Self {
        match value {
            ScholarshipRuleViolation::Forbidden
            | ScholarshipRuleViolation::ChildProfileAccessDenied => Self::Forbidden,
            ScholarshipRuleViolation::PoolClosed
            | ScholarshipRuleViolation::InsufficientPoolBalance
            | ScholarshipRuleViolation::InvalidAwardTransition
            | ScholarshipRuleViolation::MissingDisbursementTarget => {
                Self::Conflict(value.to_string())
            }
            _ => Self::Validation(value.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateScholarshipPoolInput {
    pub name: String,
    pub description: Option<String>,
    pub currency: Currency,
    pub geography_restriction: Option<String>,
    pub education_level_restriction: Option<String>,
    pub school_id_restriction: Option<Uuid>,
    pub category_restriction: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FundScholarshipPoolInput {
    pub pool_id: Uuid,
    pub amount_minor: i64,
    pub currency: Currency,
    pub external_reference: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct CreateScholarshipApplicationInput {
    pub pool_id: Uuid,
    pub child_profile_id: Uuid,
    pub student_country: Option<String>,
    pub education_level: Option<String>,
    pub school_id: Option<Uuid>,
    pub category: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DecideScholarshipApplicationInput {
    pub application_id: Uuid,
    pub amount_minor: i64,
    pub currency: Currency,
    pub decision_notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateScholarshipAwardInput {
    pub award_id: Uuid,
    pub decision_notes: Option<String>,
    pub linked_payout_request_id: Option<Uuid>,
    pub linked_vault_id: Option<Uuid>,
}

pub async fn create_scholarship_pool<W>(
    workflow: &W,
    actor: &AuthenticatedUser,
    input: CreateScholarshipPoolInput,
) -> Result<ScholarshipPool, ScholarshipError>
where
    W: ScholarshipWorkflowRepository,
{
    if !matches!(actor.role, UserRole::Donor | UserRole::PlatformAdmin) {
        return Err(ScholarshipError::Forbidden);
    }
    if input.name.trim().is_empty() {
        return Err(ScholarshipError::Validation("name is required".to_owned()));
    }

    let now = Utc::now();
    let pool = ScholarshipPool {
        id: Uuid::new_v4(),
        owner_user_id: actor.user_id,
        name: input.name,
        description: sanitize_optional(input.description),
        status: ScholarshipPoolStatus::Open,
        available_funds: Money::new(0, input.currency)
            .map_err(|err| ScholarshipError::Validation(err.to_owned()))?,
        geography_restriction: sanitize_optional(input.geography_restriction),
        education_level_restriction: sanitize_optional(input.education_level_restriction),
        school_id_restriction: input.school_id_restriction,
        category_restriction: sanitize_optional(input.category_restriction),
        created_at: now,
        updated_at: now,
    };

    let audit = scholarship_audit(
        actor.user_id,
        "scholarship_pool.created",
        pool.id,
        json!({
            "owner_user_id": pool.owner_user_id,
            "currency": pool.available_funds.currency.as_str(),
            "geography_restriction": pool.geography_restriction,
            "education_level_restriction": pool.education_level_restriction,
            "school_id_restriction": pool.school_id_restriction,
            "category_restriction": pool.category_restriction,
        }),
    );

    workflow.create_pool_with_audit(pool, audit).await.map_err(Into::into)
}

pub async fn fund_scholarship_pool<R, W>(
    pools: &R,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: FundScholarshipPoolInput,
) -> Result<(ScholarshipPool, DonorContribution), ScholarshipError>
where
    R: ScholarshipPoolRepository,
    W: ScholarshipWorkflowRepository,
{
    can_fund_pool(actor.role)?;
    validate_positive_amount(input.amount_minor)?;

    let pool = pools.find_by_id(input.pool_id).await?.ok_or(ScholarshipError::NotFound)?;
    validate_pool_is_open(&pool)?;
    validate_currency(pool.available_funds.currency.as_str(), input.currency.as_str())?;

    let now = Utc::now();
    let contribution = DonorContribution {
        id: Uuid::new_v4(),
        pool_id: pool.id,
        donor_user_id: actor.user_id,
        amount: Money::new(input.amount_minor, input.currency)
            .map_err(|err| ScholarshipError::Validation(err.to_owned()))?,
        status: DonorContributionStatus::Confirmed,
        external_reference: sanitize_optional(input.external_reference),
        idempotency_key: Some(input.idempotency_key),
        created_at: now,
        updated_at: now,
    };
    let audit = scholarship_audit(
        actor.user_id,
        "scholarship_pool.funded",
        pool.id,
        json!({
            "donor_user_id": actor.user_id,
            "amount_minor": contribution.amount.amount_minor,
            "currency": contribution.amount.currency.as_str(),
            "external_reference_present": contribution.external_reference.is_some(),
        }),
    );

    workflow.fund_pool_with_audit(contribution, audit).await.map_err(Into::into)
}

pub async fn list_scholarship_pools<R>(
    pools: &R,
    _actor: &AuthenticatedUser,
) -> Result<Vec<ScholarshipPool>, ScholarshipError>
where
    R: ScholarshipPoolRepository,
{
    pools.list().await.map_err(Into::into)
}

pub async fn get_scholarship_pool<R>(
    pools: &R,
    _actor: &AuthenticatedUser,
    pool_id: Uuid,
) -> Result<ScholarshipPool, ScholarshipError>
where
    R: ScholarshipPoolRepository,
{
    pools.find_by_id(pool_id).await?.ok_or(ScholarshipError::NotFound)
}

pub async fn create_scholarship_application<P, C, W>(
    pools: &P,
    child_profiles: &C,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: CreateScholarshipApplicationInput,
) -> Result<ScholarshipApplication, ScholarshipError>
where
    P: ScholarshipPoolRepository,
    C: ChildProfileRepository,
    W: ScholarshipWorkflowRepository,
{
    can_apply(actor.role)?;

    let pool = pools.find_by_id(input.pool_id).await?.ok_or(ScholarshipError::NotFound)?;
    validate_pool_is_open(&pool)?;

    let child = child_profiles
        .find_by_id(input.child_profile_id)
        .await?
        .ok_or(ScholarshipError::NotFound)?;
    validate_child_profile_access(actor.role, actor.user_id, child.owner_user_id)?;

    let education_level = input.education_level.clone().or_else(|| child.education_level.clone());

    validate_pool_restrictions(
        &pool,
        input.student_country.as_deref(),
        education_level.as_deref(),
        input.school_id,
        input.category.as_deref(),
    )?;

    let now = Utc::now();
    let application = ScholarshipApplication {
        id: Uuid::new_v4(),
        pool_id: pool.id,
        applicant_user_id: actor.user_id,
        child_profile_id: child.id,
        student_country: sanitize_optional(input.student_country),
        education_level: sanitize_optional(education_level),
        school_id: input.school_id,
        category: sanitize_optional(input.category),
        status: ScholarshipApplicationStatus::Submitted,
        notes: sanitize_optional(input.notes),
        created_at: now,
        updated_at: now,
    };
    let audit = scholarship_audit(
        actor.user_id,
        "scholarship_application.submitted",
        application.id,
        json!({
            "pool_id": application.pool_id,
            "child_profile_id": application.child_profile_id,
            "student_country": application.student_country,
            "education_level": application.education_level,
            "school_id": application.school_id,
            "category": application.category,
        }),
    );

    workflow.create_application_with_audit(application, audit).await.map_err(Into::into)
}

pub async fn list_pool_applications<R>(
    applications: &R,
    actor: &AuthenticatedUser,
    pool_id: Uuid,
) -> Result<Vec<ScholarshipApplication>, ScholarshipError>
where
    R: ScholarshipApplicationRepository,
{
    can_review(actor.role)?;
    applications.list_by_pool(pool_id).await.map_err(Into::into)
}

pub async fn list_pool_awards<R>(
    awards: &R,
    actor: &AuthenticatedUser,
    pool_id: Uuid,
) -> Result<Vec<ScholarshipAward>, ScholarshipError>
where
    R: ScholarshipAwardRepository,
{
    can_review(actor.role)?;
    awards.list_by_pool(pool_id).await.map_err(Into::into)
}

pub async fn list_pool_donor_contributions<R>(
    contributions: &R,
    actor: &AuthenticatedUser,
    pool_id: Uuid,
) -> Result<Vec<DonorContribution>, ScholarshipError>
where
    R: DonorContributionRepository,
{
    if !matches!(actor.role, UserRole::Donor | UserRole::PlatformAdmin) {
        return Err(ScholarshipError::Forbidden);
    }
    contributions.list_by_pool(pool_id).await.map_err(Into::into)
}

pub async fn approve_application<P, A, W>(
    pools: &P,
    applications: &A,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: DecideScholarshipApplicationInput,
) -> Result<ScholarshipReviewDecision, ScholarshipError>
where
    P: ScholarshipPoolRepository,
    A: ScholarshipApplicationRepository,
    W: ScholarshipWorkflowRepository,
{
    can_review(actor.role)?;
    validate_positive_amount(input.amount_minor)?;

    let application =
        applications.find_by_id(input.application_id).await?.ok_or(ScholarshipError::NotFound)?;
    validate_application_reviewable(application.status)?;
    let pool = pools.find_by_id(application.pool_id).await?.ok_or(ScholarshipError::NotFound)?;
    validate_currency(pool.available_funds.currency.as_str(), input.currency.as_str())?;
    ensure_pool_balance(pool.available_funds.amount_minor, input.amount_minor)?;

    let now = Utc::now();
    let award = ScholarshipAward {
        id: Uuid::new_v4(),
        application_id: application.id,
        decided_by: actor.user_id,
        amount: Money::new(input.amount_minor, input.currency)
            .map_err(|err| ScholarshipError::Validation(err.to_owned()))?,
        status: ScholarshipAwardStatus::Approved,
        decision_notes: sanitize_optional(input.decision_notes),
        linked_payout_request_id: None,
        linked_vault_id: None,
        created_at: now,
        updated_at: now,
    };

    let mut audit =
        AuditEvent::scholarship_decision(actor.user_id, application.id, &award, &pool).into_log();
    audit.action = "scholarship_application.approved".to_owned();
    audit.metadata = json!({
        "pool_id": application.pool_id,
        "award_id": award.id,
        "decision": "approved",
        "amount_minor": award.amount.amount_minor,
        "currency": award.amount.currency.as_str(),
        "decision_notes_present": award.decision_notes.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
    });

    let result = workflow
        .decide_application_with_audit(
            application.id,
            ScholarshipApplicationStatus::Approved.as_str(),
            award,
            pool.available_funds.amount_minor - input.amount_minor,
            audit,
        )
        .await?;

    Ok(result.into())
}

pub async fn reject_application<P, A, W>(
    pools: &P,
    applications: &A,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: DecideScholarshipApplicationInput,
) -> Result<ScholarshipReviewDecision, ScholarshipError>
where
    P: ScholarshipPoolRepository,
    A: ScholarshipApplicationRepository,
    W: ScholarshipWorkflowRepository,
{
    can_review(actor.role)?;
    let application =
        applications.find_by_id(input.application_id).await?.ok_or(ScholarshipError::NotFound)?;
    validate_application_reviewable(application.status)?;
    let pool = pools.find_by_id(application.pool_id).await?.ok_or(ScholarshipError::NotFound)?;

    let now = Utc::now();
    let award = ScholarshipAward {
        id: Uuid::new_v4(),
        application_id: application.id,
        decided_by: actor.user_id,
        amount: Money::new(input.amount_minor, input.currency)
            .map_err(|err| ScholarshipError::Validation(err.to_owned()))?,
        status: ScholarshipAwardStatus::Rejected,
        decision_notes: sanitize_optional(input.decision_notes),
        linked_payout_request_id: None,
        linked_vault_id: None,
        created_at: now,
        updated_at: now,
    };

    let mut audit =
        AuditEvent::scholarship_decision(actor.user_id, application.id, &award, &pool).into_log();
    audit.action = "scholarship_application.rejected".to_owned();
    audit.metadata = json!({
        "pool_id": application.pool_id,
        "award_id": award.id,
        "decision": "rejected",
        "decision_notes_present": award.decision_notes.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
    });

    let result = workflow
        .decide_application_with_audit(
            application.id,
            ScholarshipApplicationStatus::Rejected.as_str(),
            award,
            pool.available_funds.amount_minor,
            audit,
        )
        .await?;

    Ok(result.into())
}

pub async fn disburse_award<P, A, R, W>(
    pools: &P,
    applications: &A,
    awards: &R,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: UpdateScholarshipAwardInput,
) -> Result<ScholarshipReviewDecision, ScholarshipError>
where
    P: ScholarshipPoolRepository,
    A: ScholarshipApplicationRepository,
    R: ScholarshipAwardRepository,
    W: ScholarshipWorkflowRepository,
{
    can_review(actor.role)?;
    let award = awards.find_by_id(input.award_id).await?.ok_or(ScholarshipError::NotFound)?;
    let target = validate_award_transition(
        award.status,
        AwardTransition::Disburse,
        input.linked_payout_request_id,
        input.linked_vault_id,
    )?;
    let application =
        applications.find_by_id(award.application_id).await?.ok_or(ScholarshipError::NotFound)?;
    let pool = pools.find_by_id(application.pool_id).await?.ok_or(ScholarshipError::NotFound)?;
    let audit = scholarship_audit(
        actor.user_id,
        "scholarship_award.disbursed",
        award.id,
        json!({
            "pool_id": pool.id,
            "linked_payout_request_id": input.linked_payout_request_id,
            "linked_vault_id": input.linked_vault_id,
            "decision_notes_present": input.decision_notes.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false),
        }),
    );

    let result = workflow
        .transition_award_with_audit(
            award.id,
            target.as_str(),
            input.linked_payout_request_id,
            input.linked_vault_id,
            pool.available_funds.amount_minor,
            audit,
        )
        .await?;

    Ok(result.into())
}

pub async fn revoke_award<P, A, R, W>(
    pools: &P,
    applications: &A,
    awards: &R,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: UpdateScholarshipAwardInput,
) -> Result<ScholarshipReviewDecision, ScholarshipError>
where
    P: ScholarshipPoolRepository,
    A: ScholarshipApplicationRepository,
    R: ScholarshipAwardRepository,
    W: ScholarshipWorkflowRepository,
{
    can_review(actor.role)?;
    let award = awards.find_by_id(input.award_id).await?.ok_or(ScholarshipError::NotFound)?;
    let target = validate_award_transition(
        award.status,
        AwardTransition::Revoke,
        input.linked_payout_request_id,
        input.linked_vault_id,
    )?;
    let application =
        applications.find_by_id(award.application_id).await?.ok_or(ScholarshipError::NotFound)?;
    let pool = pools.find_by_id(application.pool_id).await?.ok_or(ScholarshipError::NotFound)?;
    let restored_balance = pool.available_funds.amount_minor + award.amount.amount_minor;
    let audit = scholarship_audit(
        actor.user_id,
        "scholarship_award.revoked",
        award.id,
        json!({
            "pool_id": pool.id,
            "decision_notes_present": input.decision_notes.as_ref().map(|v| !v.trim().is_empty()).unwrap_or(false),
        }),
    );

    let result = workflow
        .transition_award_with_audit(
            award.id,
            target.as_str(),
            award.linked_payout_request_id,
            award.linked_vault_id,
            restored_balance,
            audit,
        )
        .await?;

    Ok(result.into())
}

#[derive(Debug, Clone)]
pub struct ScholarshipReviewDecision {
    pub application: ScholarshipApplication,
    pub award: ScholarshipAward,
    pub pool: ScholarshipPool,
}

impl From<crate::repos::ScholarshipReviewDecisionResult> for ScholarshipReviewDecision {
    fn from(value: crate::repos::ScholarshipReviewDecisionResult) -> Self {
        Self { application: value.application, award: value.award, pool: value.pool }
    }
}

fn scholarship_audit(
    actor_user_id: Uuid,
    action: &str,
    entity_id: Uuid,
    metadata: serde_json::Value,
) -> AuditLog {
    let now = Utc::now();

    AuditLog {
        id: Uuid::new_v4(),
        actor_user_id: Some(actor_user_id),
        entity_type: "scholarship".to_owned(),
        entity_id: Some(entity_id),
        action: action.to_owned(),
        request_id: None,
        correlation_id: None,
        metadata,
        created_at: now,
        updated_at: now,
    }
}

fn sanitize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::Utc;
    use domain::{
        auth::{AuthenticatedUser, UserRole},
        persistence::{
            AuditLog, ChildProfile, DonorContribution, DonorContributionStatus,
            ScholarshipApplication, ScholarshipApplicationStatus, ScholarshipAward,
            ScholarshipAwardStatus, ScholarshipPool, ScholarshipPoolStatus,
        },
    };
    use shared::{currency::Currency, money::Money};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use crate::repos::{
        ChildProfileRepository, DonorContributionRepository, PersistenceError,
        ScholarshipApplicationRepository, ScholarshipAwardRepository, ScholarshipPoolRepository,
        ScholarshipReviewDecisionResult, ScholarshipWorkflowRepository,
    };

    use super::{
        CreateScholarshipApplicationInput, CreateScholarshipPoolInput,
        DecideScholarshipApplicationInput, FundScholarshipPoolInput, ScholarshipError,
        approve_application, create_scholarship_application, create_scholarship_pool,
        fund_scholarship_pool, revoke_award,
    };

    #[derive(Default)]
    struct FakeState {
        pools: HashMap<Uuid, ScholarshipPool>,
        child_profiles: HashMap<Uuid, ChildProfile>,
        applications: HashMap<Uuid, ScholarshipApplication>,
        awards: HashMap<Uuid, ScholarshipAward>,
        donor_contributions: Vec<DonorContribution>,
        audits: Vec<AuditLog>,
    }

    #[derive(Clone, Default)]
    struct FakeRepos {
        state: Arc<Mutex<FakeState>>,
    }

    #[async_trait]
    impl ScholarshipPoolRepository for FakeRepos {
        async fn create(&self, pool: ScholarshipPool) -> Result<ScholarshipPool, PersistenceError> {
            self.state.lock().await.pools.insert(pool.id, pool.clone());
            Ok(pool)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<ScholarshipPool>, PersistenceError> {
            Ok(self.state.lock().await.pools.get(&id).cloned())
        }
        async fn list(&self) -> Result<Vec<ScholarshipPool>, PersistenceError> {
            Ok(self.state.lock().await.pools.values().cloned().collect())
        }
    }

    #[async_trait]
    impl ChildProfileRepository for FakeRepos {
        async fn create(&self, profile: ChildProfile) -> Result<ChildProfile, PersistenceError> {
            self.state.lock().await.child_profiles.insert(profile.id, profile.clone());
            Ok(profile)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<ChildProfile>, PersistenceError> {
            Ok(self.state.lock().await.child_profiles.get(&id).cloned())
        }
        async fn list_by_owner(
            &self,
            owner_user_id: Uuid,
        ) -> Result<Vec<ChildProfile>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .child_profiles
                .values()
                .filter(|item| item.owner_user_id == owner_user_id)
                .cloned()
                .collect())
        }
    }

    #[async_trait]
    impl ScholarshipApplicationRepository for FakeRepos {
        async fn create(
            &self,
            application: ScholarshipApplication,
        ) -> Result<ScholarshipApplication, PersistenceError> {
            self.state.lock().await.applications.insert(application.id, application.clone());
            Ok(application)
        }
        async fn list_by_pool(
            &self,
            pool_id: Uuid,
        ) -> Result<Vec<ScholarshipApplication>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .applications
                .values()
                .filter(|item| item.pool_id == pool_id)
                .cloned()
                .collect())
        }
        async fn find_by_id(
            &self,
            id: Uuid,
        ) -> Result<Option<ScholarshipApplication>, PersistenceError> {
            Ok(self.state.lock().await.applications.get(&id).cloned())
        }
    }

    #[async_trait]
    impl ScholarshipAwardRepository for FakeRepos {
        async fn create(
            &self,
            award: ScholarshipAward,
        ) -> Result<ScholarshipAward, PersistenceError> {
            self.state.lock().await.awards.insert(award.id, award.clone());
            Ok(award)
        }
        async fn list_by_pool(
            &self,
            pool_id: Uuid,
        ) -> Result<Vec<ScholarshipAward>, PersistenceError> {
            let state = self.state.lock().await;
            let application_ids: Vec<Uuid> = state
                .applications
                .values()
                .filter(|app| app.pool_id == pool_id)
                .map(|app| app.id)
                .collect();
            Ok(state
                .awards
                .values()
                .filter(|award| application_ids.contains(&award.application_id))
                .cloned()
                .collect())
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<ScholarshipAward>, PersistenceError> {
            Ok(self.state.lock().await.awards.get(&id).cloned())
        }
    }

    #[async_trait]
    impl DonorContributionRepository for FakeRepos {
        async fn create(
            &self,
            contribution: DonorContribution,
        ) -> Result<DonorContribution, PersistenceError> {
            self.state.lock().await.donor_contributions.push(contribution.clone());
            Ok(contribution)
        }
        async fn list_by_pool(
            &self,
            pool_id: Uuid,
        ) -> Result<Vec<DonorContribution>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .donor_contributions
                .iter()
                .filter(|item| item.pool_id == pool_id)
                .cloned()
                .collect())
        }
    }

    #[async_trait]
    impl ScholarshipWorkflowRepository for FakeRepos {
        async fn create_pool_with_audit(
            &self,
            pool: ScholarshipPool,
            audit_log: AuditLog,
        ) -> Result<ScholarshipPool, PersistenceError> {
            let mut state = self.state.lock().await;
            state.pools.insert(pool.id, pool.clone());
            state.audits.push(audit_log);
            Ok(pool)
        }
        async fn fund_pool_with_audit(
            &self,
            contribution: DonorContribution,
            audit_log: AuditLog,
        ) -> Result<(ScholarshipPool, DonorContribution), PersistenceError> {
            let mut state = self.state.lock().await;
            let pool_snapshot = {
                let pool =
                    state.pools.get_mut(&contribution.pool_id).ok_or(PersistenceError::NotFound)?;
                pool.available_funds.amount_minor += contribution.amount.amount_minor;
                pool.updated_at = Utc::now();
                pool.clone()
            };
            state.donor_contributions.push(contribution.clone());
            state.audits.push(audit_log);
            Ok((pool_snapshot, contribution))
        }
        async fn create_application_with_audit(
            &self,
            application: ScholarshipApplication,
            audit_log: AuditLog,
        ) -> Result<ScholarshipApplication, PersistenceError> {
            let mut state = self.state.lock().await;
            state.applications.insert(application.id, application.clone());
            state.audits.push(audit_log);
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
            let mut state = self.state.lock().await;
            let (application_snapshot, pool_id) = {
                let application = state
                    .applications
                    .get_mut(&application_id)
                    .ok_or(PersistenceError::NotFound)?;
                application.status = application_status.parse().map_err(|_| {
                    PersistenceError::Validation(
                        "invalid scholarship application status".to_owned(),
                    )
                })?;
                (application.clone(), application.pool_id)
            };
            let pool_snapshot = {
                let pool = state.pools.get_mut(&pool_id).ok_or(PersistenceError::NotFound)?;
                pool.available_funds.amount_minor = pool_available_funds_minor;
                pool.updated_at = Utc::now();
                pool.clone()
            };
            state.awards.insert(award.id, award.clone());
            state.audits.push(audit_log);
            Ok(ScholarshipReviewDecisionResult {
                application: application_snapshot,
                award,
                pool: pool_snapshot,
            })
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
            let mut state = self.state.lock().await;
            let mut award =
                state.awards.get(&award_id).cloned().ok_or(PersistenceError::NotFound)?;
            award.status = status.parse().map_err(|_| {
                PersistenceError::Validation("invalid scholarship award status".to_owned())
            })?;
            award.linked_payout_request_id = linked_payout_request_id;
            award.linked_vault_id = linked_vault_id;
            award.updated_at = Utc::now();
            state.awards.insert(award.id, award.clone());
            let application = state
                .applications
                .get(&award.application_id)
                .cloned()
                .ok_or(PersistenceError::NotFound)?;
            let pool = {
                let pool =
                    state.pools.get_mut(&application.pool_id).ok_or(PersistenceError::NotFound)?;
                pool.available_funds.amount_minor = pool_available_funds_minor;
                pool.updated_at = Utc::now();
                pool.clone()
            };
            state.audits.push(audit_log);
            Ok(ScholarshipReviewDecisionResult { application, award, pool })
        }
    }

    #[tokio::test]
    async fn donor_can_create_and_fund_pool() {
        let repos = seeded_repos().await;
        let donor = actor(UserRole::Donor, owner_id());

        let pool = create_scholarship_pool(
            &repos,
            &donor,
            CreateScholarshipPoolInput {
                name: "STEM Fund".to_owned(),
                description: None,
                currency: usd(),
                geography_restriction: Some("NG".to_owned()),
                education_level_restriction: None,
                school_id_restriction: None,
                category_restriction: Some("stem".to_owned()),
            },
        )
        .await
        .unwrap();

        let (updated_pool, contribution) = fund_scholarship_pool(
            &repos,
            &repos,
            &donor,
            FundScholarshipPoolInput {
                pool_id: pool.id,
                amount_minor: 50_000,
                currency: usd(),
                external_reference: None,
                idempotency_key: "fund-1".to_owned(),
            },
        )
        .await
        .unwrap();

        assert_eq!(updated_pool.available_funds.amount_minor, 50_000);
        assert_eq!(contribution.status, DonorContributionStatus::Confirmed);
    }

    #[tokio::test]
    async fn application_must_match_pool_restrictions() {
        let repos = seeded_repos().await;
        let parent = actor(UserRole::Parent, owner_id());

        let error = create_scholarship_application(
            &repos,
            &repos,
            &repos,
            &parent,
            CreateScholarshipApplicationInput {
                pool_id: seeded_pool_id(),
                child_profile_id: seeded_child_id(),
                student_country: Some("GH".to_owned()),
                education_level: Some("secondary".to_owned()),
                school_id: Some(seeded_school_id()),
                category: Some("stem".to_owned()),
                notes: None,
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, ScholarshipError::Validation(_)));
    }

    #[tokio::test]
    async fn approve_cannot_exceed_pool_balance() {
        let repos = seeded_repos().await;
        let admin = actor(UserRole::PlatformAdmin, Uuid::new_v4());

        let error = approve_application(
            &repos,
            &repos,
            &repos,
            &admin,
            DecideScholarshipApplicationInput {
                application_id: seeded_application_id(),
                amount_minor: 100_000,
                currency: usd(),
                decision_notes: None,
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, ScholarshipError::Conflict(_)));
    }

    #[tokio::test]
    async fn revoking_approved_award_restores_pool_balance() {
        let repos = seeded_repos().await;
        let admin = actor(UserRole::PlatformAdmin, Uuid::new_v4());

        let decision = approve_application(
            &repos,
            &repos,
            &repos,
            &admin,
            DecideScholarshipApplicationInput {
                application_id: seeded_application_id(),
                amount_minor: 10_000,
                currency: usd(),
                decision_notes: Some("approved".to_owned()),
            },
        )
        .await
        .unwrap();

        let revoked = revoke_award(
            &repos,
            &repos,
            &repos,
            &repos,
            &admin,
            super::UpdateScholarshipAwardInput {
                award_id: decision.award.id,
                decision_notes: Some("revoked".to_owned()),
                linked_payout_request_id: None,
                linked_vault_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(revoked.award.status, ScholarshipAwardStatus::Revoked);
        assert_eq!(revoked.pool.available_funds.amount_minor, 25_000);
    }

    async fn seeded_repos() -> FakeRepos {
        let repos = FakeRepos::default();
        let now = Utc::now();
        {
            let mut state = repos.state.lock().await;
            state.pools.insert(
                seeded_pool_id(),
                ScholarshipPool {
                    id: seeded_pool_id(),
                    owner_user_id: owner_id(),
                    name: "Seed Pool".to_owned(),
                    description: None,
                    status: ScholarshipPoolStatus::Open,
                    available_funds: Money::new(25_000, usd()).unwrap(),
                    geography_restriction: Some("NG".to_owned()),
                    education_level_restriction: Some("secondary".to_owned()),
                    school_id_restriction: Some(seeded_school_id()),
                    category_restriction: Some("stem".to_owned()),
                    created_at: now,
                    updated_at: now,
                },
            );
            state.child_profiles.insert(
                seeded_child_id(),
                ChildProfile {
                    id: seeded_child_id(),
                    owner_user_id: owner_id(),
                    full_name: "Student".to_owned(),
                    date_of_birth: None,
                    education_level: Some("secondary".to_owned()),
                    created_at: now,
                    updated_at: now,
                },
            );
            state.applications.insert(
                seeded_application_id(),
                ScholarshipApplication {
                    id: seeded_application_id(),
                    pool_id: seeded_pool_id(),
                    applicant_user_id: owner_id(),
                    child_profile_id: seeded_child_id(),
                    student_country: Some("NG".to_owned()),
                    education_level: Some("secondary".to_owned()),
                    school_id: Some(seeded_school_id()),
                    category: Some("stem".to_owned()),
                    status: ScholarshipApplicationStatus::Submitted,
                    notes: None,
                    created_at: now,
                    updated_at: now,
                },
            );
        }
        repos
    }

    fn actor(role: UserRole, user_id: Uuid) -> AuthenticatedUser {
        AuthenticatedUser { user_id, role, session_id: Uuid::new_v4() }
    }

    fn owner_id() -> Uuid {
        Uuid::from_u128(10)
    }

    fn seeded_pool_id() -> Uuid {
        Uuid::from_u128(11)
    }

    fn seeded_child_id() -> Uuid {
        Uuid::from_u128(12)
    }

    fn seeded_application_id() -> Uuid {
        Uuid::from_u128(13)
    }

    fn seeded_school_id() -> Uuid {
        Uuid::from_u128(14)
    }

    fn usd() -> Currency {
        Currency::Fiat("USD".to_owned())
    }
}
