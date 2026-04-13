use chrono::{NaiveDate, Utc};
use domain::{
    auth::{AuthenticatedUser, UserRole},
    persistence::{
        Milestone, MilestoneStatus, PayoutType, PlanStatus, SavingsPlan, VaultContributor,
    },
};
use shared::{currency::Currency, money::Money};
use uuid::Uuid;

use crate::{
    audit::{AuditEvent, AuditService},
    repos::{
        AuditLogRepository, ChildProfileRepository, MilestoneRepository, PersistenceError,
        SavingsPlanRepository, VaultRepository,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum SavingsError {
    #[error("{0}")]
    Validation(String),
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("repository error: {0}")]
    Repository(String),
}

impl From<PersistenceError> for SavingsError {
    fn from(value: PersistenceError) -> Self {
        match value {
            PersistenceError::NotFound => Self::NotFound,
            PersistenceError::Conflict(message) | PersistenceError::Validation(message) => {
                Self::Validation(message)
            }
            PersistenceError::Repository(message) => Self::Repository(message),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreatePlanInput {
    pub child_profile_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub target_amount_minor: i64,
    pub currency: Currency,
}

#[derive(Debug, Clone)]
pub struct CreateMilestoneInput {
    pub vault_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub due_date: NaiveDate,
    pub target_amount_minor: i64,
    pub currency: Currency,
    pub payout_type: PayoutType,
}

#[derive(Debug, Clone)]
pub struct AddContributorInput {
    pub vault_id: Uuid,
    pub contributor_user_id: Uuid,
    pub role_label: String,
}

pub async fn create_plan<P, C, A>(
    plans: &P,
    child_profiles: &C,
    audits: &A,
    actor: &AuthenticatedUser,
    input: CreatePlanInput,
) -> Result<SavingsPlan, SavingsError>
where
    P: SavingsPlanRepository,
    C: ChildProfileRepository,
    A: AuditLogRepository,
{
    if actor.role != UserRole::Parent {
        return Err(SavingsError::Forbidden);
    }
    if input.name.trim().is_empty() {
        return Err(SavingsError::Validation("name is required".to_owned()));
    }
    let child =
        child_profiles.find_by_id(input.child_profile_id).await?.ok_or(SavingsError::NotFound)?;
    if child.owner_user_id != actor.user_id {
        return Err(SavingsError::Forbidden);
    }

    let now = Utc::now();
    let plan = SavingsPlan {
        id: Uuid::new_v4(),
        child_profile_id: input.child_profile_id,
        owner_user_id: actor.user_id,
        name: input.name.trim().to_owned(),
        description: sanitize_optional(input.description),
        target_amount: Money::new(input.target_amount_minor, input.currency)
            .map_err(|message| SavingsError::Validation(message.to_owned()))?,
        status: PlanStatus::Draft,
        created_at: now,
        updated_at: now,
    };

    let saved = plans.create(plan).await?;
    AuditService::new(audits)
        .record(AuditEvent::plan_created(actor.user_id, &saved))
        .await
        .map_err(|error| SavingsError::Repository(error.to_string()))?;
    Ok(saved)
}

pub async fn create_milestone<M, V, A>(
    milestones: &M,
    vaults: &V,
    audits: &A,
    actor: &AuthenticatedUser,
    input: CreateMilestoneInput,
) -> Result<Milestone, SavingsError>
where
    M: MilestoneRepository,
    V: VaultRepository,
    A: AuditLogRepository,
{
    let vault = vaults.find_by_id(input.vault_id).await?.ok_or(SavingsError::NotFound)?;
    if vault.owner_user_id != actor.user_id {
        return Err(SavingsError::Forbidden);
    }
    if input.title.trim().is_empty() {
        return Err(SavingsError::Validation("title is required".to_owned()));
    }
    if vault.currency != input.currency.as_str() {
        return Err(SavingsError::Validation("milestone currency must match vault".to_owned()));
    }

    let now = Utc::now();
    let milestone = Milestone {
        id: Uuid::new_v4(),
        vault_id: input.vault_id,
        title: input.title.trim().to_owned(),
        description: sanitize_optional(input.description),
        due_date: input.due_date,
        target_amount: Money::new(input.target_amount_minor, input.currency)
            .map_err(|message| SavingsError::Validation(message.to_owned()))?,
        funded_amount: Money::new(0, Currency::Fiat(vault.currency.clone()))
            .map_err(|message| SavingsError::Validation(message.to_owned()))?,
        payout_type: input.payout_type,
        status: MilestoneStatus::Planned,
        created_at: now,
        updated_at: now,
    };

    let saved = milestones.create(milestone).await?;
    AuditService::new(audits)
        .record(AuditEvent::milestone_created(actor.user_id, &saved))
        .await
        .map_err(|error| SavingsError::Repository(error.to_string()))?;
    Ok(saved)
}

pub async fn add_contributor<V, A>(
    vaults: &V,
    audits: &A,
    actor: &AuthenticatedUser,
    input: AddContributorInput,
) -> Result<VaultContributor, SavingsError>
where
    V: VaultRepository,
    A: AuditLogRepository,
{
    let vault = vaults.find_by_id(input.vault_id).await?.ok_or(SavingsError::NotFound)?;
    if vault.owner_user_id != actor.user_id {
        return Err(SavingsError::Forbidden);
    }
    if input.role_label.trim().is_empty() {
        return Err(SavingsError::Validation("role_label is required".to_owned()));
    }

    let now = Utc::now();
    let contributor = VaultContributor {
        id: Uuid::new_v4(),
        vault_id: input.vault_id,
        contributor_user_id: input.contributor_user_id,
        role_label: input.role_label.trim().to_owned(),
        created_at: now,
        updated_at: now,
    };

    let saved = vaults.add_contributor(contributor).await?;
    AuditService::new(audits)
        .record(AuditEvent::contributor_added(actor.user_id, &saved))
        .await
        .map_err(|error| SavingsError::Repository(error.to_string()))?;
    Ok(saved)
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
    use domain::persistence::{ChildProfile, SavingsVault, VaultStatus};
    use tokio::sync::Mutex;

    use super::*;
    use crate::repos::AuditLogRepository;

    #[derive(Default, Clone)]
    struct FakePlans {
        items: Arc<Mutex<HashMap<Uuid, SavingsPlan>>>,
    }

    #[async_trait]
    impl SavingsPlanRepository for FakePlans {
        async fn create(&self, plan: SavingsPlan) -> Result<SavingsPlan, PersistenceError> {
            self.items.lock().await.insert(plan.id, plan.clone());
            Ok(plan)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<SavingsPlan>, PersistenceError> {
            Ok(self.items.lock().await.get(&id).cloned())
        }
        async fn list_by_child_profile(
            &self,
            child_profile_id: Uuid,
        ) -> Result<Vec<SavingsPlan>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .await
                .values()
                .filter(|item| item.child_profile_id == child_profile_id)
                .cloned()
                .collect())
        }
    }

    #[derive(Default, Clone)]
    struct FakeChildren {
        items: Arc<Mutex<HashMap<Uuid, ChildProfile>>>,
    }

    #[async_trait]
    impl ChildProfileRepository for FakeChildren {
        async fn create(&self, profile: ChildProfile) -> Result<ChildProfile, PersistenceError> {
            self.items.lock().await.insert(profile.id, profile.clone());
            Ok(profile)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<ChildProfile>, PersistenceError> {
            Ok(self.items.lock().await.get(&id).cloned())
        }
        async fn list_by_owner(
            &self,
            owner_user_id: Uuid,
        ) -> Result<Vec<ChildProfile>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .await
                .values()
                .filter(|item| item.owner_user_id == owner_user_id)
                .cloned()
                .collect())
        }
    }

    #[derive(Default, Clone)]
    struct FakeVaults {
        vaults: Arc<Mutex<HashMap<Uuid, SavingsVault>>>,
        contributors: Arc<Mutex<HashMap<Uuid, VaultContributor>>>,
    }

    #[async_trait]
    impl VaultRepository for FakeVaults {
        async fn create(&self, vault: SavingsVault) -> Result<SavingsVault, PersistenceError> {
            self.vaults.lock().await.insert(vault.id, vault.clone());
            Ok(vault)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<SavingsVault>, PersistenceError> {
            Ok(self.vaults.lock().await.get(&id).cloned())
        }
        async fn update_balances(
            &self,
            _id: Uuid,
            _total_contributed_minor: i64,
            _total_locked_minor: i64,
            _expected_version: i64,
        ) -> Result<(), PersistenceError> {
            Ok(())
        }
        async fn add_contributor(
            &self,
            contributor: VaultContributor,
        ) -> Result<VaultContributor, PersistenceError> {
            self.contributors.lock().await.insert(contributor.id, contributor.clone());
            Ok(contributor)
        }
        async fn list_contributors(
            &self,
            _vault_id: Uuid,
        ) -> Result<Vec<VaultContributor>, PersistenceError> {
            Ok(self.contributors.lock().await.values().cloned().collect())
        }
    }

    #[derive(Default, Clone)]
    struct FakeMilestones {
        items: Arc<Mutex<HashMap<Uuid, Milestone>>>,
    }

    #[async_trait]
    impl MilestoneRepository for FakeMilestones {
        async fn create(&self, milestone: Milestone) -> Result<Milestone, PersistenceError> {
            self.items.lock().await.insert(milestone.id, milestone.clone());
            Ok(milestone)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<Milestone>, PersistenceError> {
            Ok(self.items.lock().await.get(&id).cloned())
        }
        async fn list_by_vault(&self, vault_id: Uuid) -> Result<Vec<Milestone>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .await
                .values()
                .filter(|item| item.vault_id == vault_id)
                .cloned()
                .collect())
        }
        async fn update_funded_amount(
            &self,
            _id: Uuid,
            _funded_amount_minor: i64,
            _status: &str,
        ) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    #[derive(Default, Clone)]
    struct FakeAudits {
        items: Arc<Mutex<Vec<domain::persistence::AuditLog>>>,
    }

    #[async_trait]
    impl AuditLogRepository for FakeAudits {
        async fn append(
            &self,
            audit_log: domain::persistence::AuditLog,
        ) -> Result<domain::persistence::AuditLog, PersistenceError> {
            self.items.lock().await.push(audit_log.clone());
            Ok(audit_log)
        }
        async fn list_by_entity(
            &self,
            _entity_type: &str,
            _entity_id: Uuid,
        ) -> Result<Vec<domain::persistence::AuditLog>, PersistenceError> {
            Ok(vec![])
        }
        async fn list_by_actor(
            &self,
            _actor_user_id: Uuid,
        ) -> Result<Vec<domain::persistence::AuditLog>, PersistenceError> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn create_plan_and_add_contributor_emit_audits() {
        let plans = FakePlans::default();
        let children = FakeChildren::default();
        let vaults = FakeVaults::default();
        let milestones = FakeMilestones::default();
        let audits = FakeAudits::default();
        let actor = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            role: UserRole::Parent,
            session_id: Uuid::new_v4(),
        };
        let child = ChildProfile {
            id: Uuid::new_v4(),
            owner_user_id: actor.user_id,
            full_name: "Student".to_owned(),
            date_of_birth: None,
            education_level: Some("secondary".to_owned()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        children.create(child.clone()).await.unwrap();
        let vault = SavingsVault {
            id: Uuid::new_v4(),
            plan_id: Uuid::new_v4(),
            owner_user_id: actor.user_id,
            currency: "USD".to_owned(),
            status: VaultStatus::Active,
            total_contributed_minor: 0,
            total_locked_minor: 0,
            total_disbursed_minor: 0,
            external_wallet_account_id: None,
            external_contract_ref: None,
            version: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        vaults.create(vault.clone()).await.unwrap();

        let plan = create_plan(
            &plans,
            &children,
            &audits,
            &actor,
            CreatePlanInput {
                child_profile_id: child.id,
                name: "School Fees".to_owned(),
                description: None,
                target_amount_minor: 10_000,
                currency: Currency::Fiat("USD".to_owned()),
            },
        )
        .await
        .unwrap();

        let _milestone = create_milestone(
            &milestones,
            &vaults,
            &audits,
            &actor,
            CreateMilestoneInput {
                vault_id: vault.id,
                title: "Term 1".to_owned(),
                description: None,
                due_date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
                target_amount_minor: 5_000,
                currency: Currency::Fiat("USD".to_owned()),
                payout_type: PayoutType::Tuition,
            },
        )
        .await
        .unwrap();

        let _contributor = add_contributor(
            &vaults,
            &audits,
            &actor,
            AddContributorInput {
                vault_id: vault.id,
                contributor_user_id: Uuid::new_v4(),
                role_label: "aunt".to_owned(),
            },
        )
        .await
        .unwrap();

        let logged = audits.items.lock().await;
        assert_eq!(plan.name, "School Fees");
        assert_eq!(logged.len(), 3);
        assert_eq!(logged[0].action, "savings_plan.created");
        assert_eq!(logged[1].action, "milestone.created");
        assert_eq!(logged[2].action, "vault_contributor.added");
    }
}
