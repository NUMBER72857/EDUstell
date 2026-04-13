use chrono::Utc;
use domain::{
    auth::{AuthenticatedUser, UserRole},
    contributions::{
        ContributionRuleViolation, ContributionTransition, can_settle_contribution,
        can_view_or_fund_vault, validate_contribution_amount, validate_contribution_currency,
        validate_transition,
    },
    persistence::{Contribution, ContributionSourceType, ContributionStatus},
};
use serde_json::json;
use shared::{currency::Currency, money::Money};
use uuid::Uuid;

use crate::audit::AuditEvent;
use crate::repos::{
    ContributionRepository, ContributionWorkflowRepository, PersistenceError, VaultRepository,
};

#[derive(Debug, thiserror::Error)]
pub enum ContributionError {
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

impl From<PersistenceError> for ContributionError {
    fn from(value: PersistenceError) -> Self {
        match value {
            PersistenceError::NotFound => Self::NotFound,
            PersistenceError::Conflict(message) => Self::Conflict(message),
            PersistenceError::Validation(message) => Self::Validation(message),
            PersistenceError::Repository(message) => Self::Repository(message),
        }
    }
}

impl From<ContributionRuleViolation> for ContributionError {
    fn from(value: ContributionRuleViolation) -> Self {
        match value {
            ContributionRuleViolation::UnauthorizedContributor
            | ContributionRuleViolation::UnauthorizedSettlementActor => Self::Forbidden,
            ContributionRuleViolation::InvalidStatusTransition { .. } => {
                Self::Conflict(value.to_string())
            }
            _ => Self::Validation(value.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RecordContributionIntentInput {
    pub vault_id: Uuid,
    pub amount_minor: i64,
    pub currency: Currency,
    pub source_type: ContributionSourceType,
    pub external_reference: Option<String>,
    pub idempotency_key: String,
}

#[derive(Debug, Clone)]
pub struct SettlementInput {
    pub contribution_id: Uuid,
    pub external_reference: Option<String>,
}

pub async fn record_contribution_intent<V, C, W>(
    vaults: &V,
    contributions: &C,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: RecordContributionIntentInput,
) -> Result<Contribution, ContributionError>
where
    V: VaultRepository,
    C: ContributionRepository,
    W: ContributionWorkflowRepository,
{
    if !matches!(actor.role, UserRole::Parent | UserRole::Contributor) {
        return Err(ContributionError::Forbidden);
    }

    validate_contribution_amount(input.amount_minor)?;

    let vault = vaults.find_by_id(input.vault_id).await?.ok_or(ContributionError::NotFound)?;
    let contributors = vaults.list_contributors(input.vault_id).await?;
    let is_explicit_contributor =
        contributors.iter().any(|item| item.contributor_user_id == actor.user_id);

    can_view_or_fund_vault(
        actor.role,
        actor.user_id,
        vault.owner_user_id,
        is_explicit_contributor,
    )?;
    validate_contribution_currency(input.currency.as_str(), &vault.currency)?;

    if let Some(existing) = contributions.find_by_idempotency_key(&input.idempotency_key).await? {
        ensure_same_intent(&existing, actor.user_id, &input)?;
        return Ok(existing);
    }

    let now = Utc::now();
    let contribution = Contribution {
        id: Uuid::new_v4(),
        vault_id: input.vault_id,
        contributor_user_id: actor.user_id,
        amount: Money::new(input.amount_minor, input.currency.clone())
            .map_err(|message| ContributionError::Validation(message.to_owned()))?,
        status: ContributionStatus::Pending,
        source_type: input.source_type,
        external_reference: input.external_reference.clone(),
        idempotency_key: Some(input.idempotency_key.clone()),
        created_at: now,
        updated_at: now,
    };
    let mut audit = AuditEvent::contribution_recorded(actor.user_id, &contribution).into_log();
    audit.metadata = json!({
        "vault_id": contribution.vault_id,
        "contributor_user_id": contribution.contributor_user_id,
        "amount_minor": contribution.amount.amount_minor,
        "currency": contribution.amount.currency.as_str(),
        "source_type": contribution.source_type.as_str(),
        "status": contribution.status.as_str(),
        "idempotency_key": input.idempotency_key,
    });

    workflow.create_intent_with_audit(contribution, audit).await.map_err(Into::into)
}

pub async fn list_vault_contributions<V, C>(
    vaults: &V,
    contributions: &C,
    actor: &AuthenticatedUser,
    vault_id: Uuid,
) -> Result<Vec<Contribution>, ContributionError>
where
    V: VaultRepository,
    C: ContributionRepository,
{
    let vault = vaults.find_by_id(vault_id).await?.ok_or(ContributionError::NotFound)?;
    let contributors = vaults.list_contributors(vault_id).await?;
    let is_explicit_contributor =
        contributors.iter().any(|item| item.contributor_user_id == actor.user_id);

    can_view_or_fund_vault(
        actor.role,
        actor.user_id,
        vault.owner_user_id,
        is_explicit_contributor,
    )?;

    contributions.list_by_vault(vault_id).await.map_err(Into::into)
}

pub async fn confirm_contribution<C, W>(
    contributions: &C,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: SettlementInput,
) -> Result<Contribution, ContributionError>
where
    C: ContributionRepository,
    W: ContributionWorkflowRepository,
{
    settle_contribution(
        contributions,
        workflow,
        actor,
        input,
        ContributionTransition::Confirm,
        "contribution.confirmed",
    )
    .await
}

pub async fn fail_contribution<C, W>(
    contributions: &C,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: SettlementInput,
) -> Result<Contribution, ContributionError>
where
    C: ContributionRepository,
    W: ContributionWorkflowRepository,
{
    settle_contribution(
        contributions,
        workflow,
        actor,
        input,
        ContributionTransition::Fail,
        "contribution.failed",
    )
    .await
}

pub async fn reverse_contribution<C, W>(
    contributions: &C,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: SettlementInput,
) -> Result<Contribution, ContributionError>
where
    C: ContributionRepository,
    W: ContributionWorkflowRepository,
{
    settle_contribution(
        contributions,
        workflow,
        actor,
        input,
        ContributionTransition::Reverse,
        "contribution.reversed",
    )
    .await
}

async fn settle_contribution<C, W>(
    contributions: &C,
    workflow: &W,
    actor: &AuthenticatedUser,
    input: SettlementInput,
    transition: ContributionTransition,
    audit_action: &'static str,
) -> Result<Contribution, ContributionError>
where
    C: ContributionRepository,
    W: ContributionWorkflowRepository,
{
    can_settle_contribution(actor.role)?;

    let current = contributions
        .find_by_id(input.contribution_id)
        .await?
        .ok_or(ContributionError::NotFound)?;
    let target_status = validate_transition(current.status, transition)?;

    if current.status == target_status {
        return Ok(current);
    }

    let mut audit = AuditEvent::contribution_status_changed(
        actor.user_id,
        &current,
        target_status.as_str(),
        input.external_reference.is_some(),
    )
    .into_log();
    audit.action = audit_action.to_owned();

    let result = workflow
        .settle_with_audit(
            current.id,
            target_status.as_str(),
            actor.user_id,
            input.external_reference.as_deref(),
            audit,
        )
        .await?;

    Ok(result.contribution)
}

fn ensure_same_intent(
    existing: &Contribution,
    actor_user_id: Uuid,
    input: &RecordContributionIntentInput,
) -> Result<(), ContributionError> {
    if existing.vault_id != input.vault_id
        || existing.contributor_user_id != actor_user_id
        || existing.amount.amount_minor != input.amount_minor
        || existing.amount.currency != input.currency
        || existing.source_type != input.source_type
    {
        return Err(ContributionError::Conflict(
            "idempotency key already used for a different contribution intent".to_owned(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::Utc;
    use domain::{
        auth::{AuthenticatedUser, UserRole},
        contributions::{VaultLedgerEntry, VaultLedgerEntryType},
        persistence::{
            AuditLog, Contribution, ContributionSourceType, ContributionStatus, SavingsVault,
            VaultContributor, VaultStatus,
        },
    };
    use shared::currency::Currency;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    use crate::repos::{
        ContributionRepository, ContributionSettlementResult, ContributionWorkflowRepository,
        PersistenceError, VaultRepository,
    };

    use super::{
        ContributionError, RecordContributionIntentInput, SettlementInput, confirm_contribution,
        list_vault_contributions, record_contribution_intent, reverse_contribution,
    };

    #[derive(Default)]
    struct FakeState {
        vaults: HashMap<Uuid, SavingsVault>,
        contributors: HashMap<Uuid, Vec<VaultContributor>>,
        contributions: HashMap<Uuid, Contribution>,
        idempotency: HashMap<String, Uuid>,
        audits: Vec<AuditLog>,
        ledger_entries: Vec<VaultLedgerEntry>,
    }

    #[derive(Clone, Default)]
    struct FakeRepos {
        state: Arc<Mutex<FakeState>>,
    }

    #[async_trait]
    impl VaultRepository for FakeRepos {
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
            vault_id: Uuid,
        ) -> Result<Vec<VaultContributor>, PersistenceError> {
            Ok(self.state.lock().await.contributors.get(&vault_id).cloned().unwrap_or_default())
        }
    }

    #[async_trait]
    impl ContributionRepository for FakeRepos {
        async fn create(&self, _: Contribution) -> Result<Contribution, PersistenceError> {
            unimplemented!()
        }

        async fn find_by_id(&self, id: Uuid) -> Result<Option<Contribution>, PersistenceError> {
            Ok(self.state.lock().await.contributions.get(&id).cloned())
        }

        async fn find_by_idempotency_key(
            &self,
            idempotency_key: &str,
        ) -> Result<Option<Contribution>, PersistenceError> {
            let state = self.state.lock().await;
            let Some(id) = state.idempotency.get(idempotency_key) else {
                return Ok(None);
            };
            Ok(state.contributions.get(id).cloned())
        }

        async fn list_by_vault(
            &self,
            vault_id: Uuid,
        ) -> Result<Vec<Contribution>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .contributions
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
        ) -> Result<(), PersistenceError> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl ContributionWorkflowRepository for FakeRepos {
        async fn create_intent_with_audit(
            &self,
            contribution: Contribution,
            audit_log: AuditLog,
        ) -> Result<Contribution, PersistenceError> {
            let mut state = self.state.lock().await;
            if let Some(key) = contribution.idempotency_key.clone() {
                state.idempotency.insert(key, contribution.id);
            }
            state.contributions.insert(contribution.id, contribution.clone());
            state.audits.push(audit_log);
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
            let mut state = self.state.lock().await;
            let target = match status {
                "confirmed" => ContributionStatus::Confirmed,
                "failed" => ContributionStatus::Failed,
                "reversed" => ContributionStatus::Reversed,
                _ => return Err(PersistenceError::Validation("invalid status".to_owned())),
            };

            let mut contribution = state
                .contributions
                .get(&contribution_id)
                .cloned()
                .ok_or(PersistenceError::NotFound)?;

            if contribution.status == target {
                let vault = state
                    .vaults
                    .get(&contribution.vault_id)
                    .cloned()
                    .ok_or(PersistenceError::NotFound)?;
                return Ok(ContributionSettlementResult {
                    contribution,
                    ledger_entry: None,
                    vault,
                });
            }

            contribution.status = target;
            contribution.external_reference = external_reference
                .map(ToOwned::to_owned)
                .or_else(|| contribution.external_reference.clone());
            contribution.updated_at = Utc::now();

            let mut ledger_entry = None;
            match target {
                ContributionStatus::Confirmed => {
                    let vault = state
                        .vaults
                        .get_mut(&contribution.vault_id)
                        .ok_or(PersistenceError::NotFound)?;
                    vault.total_contributed_minor += contribution.amount.amount_minor;
                    vault.version += 1;
                    ledger_entry = Some(VaultLedgerEntry {
                        id: Uuid::new_v4(),
                        vault_id: vault.id,
                        contribution_id: Some(contribution.id),
                        actor_user_id: Some(actor_user_id),
                        entry_type: VaultLedgerEntryType::ContributionConfirmed,
                        amount: contribution.amount.clone(),
                        balance_after_minor: vault.total_contributed_minor,
                        external_reference: contribution.external_reference.clone(),
                        metadata: serde_json::json!({}),
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    });
                }
                ContributionStatus::Reversed => {
                    let vault = state
                        .vaults
                        .get_mut(&contribution.vault_id)
                        .ok_or(PersistenceError::NotFound)?;
                    vault.total_contributed_minor -= contribution.amount.amount_minor;
                    vault.version += 1;
                    ledger_entry = Some(VaultLedgerEntry {
                        id: Uuid::new_v4(),
                        vault_id: vault.id,
                        contribution_id: Some(contribution.id),
                        actor_user_id: Some(actor_user_id),
                        entry_type: VaultLedgerEntryType::ContributionReversed,
                        amount: contribution.amount.clone(),
                        balance_after_minor: vault.total_contributed_minor,
                        external_reference: contribution.external_reference.clone(),
                        metadata: serde_json::json!({}),
                        created_at: Utc::now(),
                        updated_at: Utc::now(),
                    });
                }
                ContributionStatus::Failed | ContributionStatus::Pending => {}
            }

            state.contributions.insert(contribution_id, contribution.clone());
            if let Some(entry) = ledger_entry.clone() {
                state.ledger_entries.push(entry);
            }
            state.audits.push(audit_log);
            let vault = state
                .vaults
                .get(&contribution.vault_id)
                .cloned()
                .ok_or(PersistenceError::NotFound)?;

            Ok(ContributionSettlementResult { contribution, ledger_entry, vault })
        }
    }

    #[tokio::test]
    async fn record_contribution_is_idempotent_for_same_intent() {
        let repos = seeded_repos().await;
        let actor = contributor_user();
        let vault_id = seeded_vault_id();

        let first = record_contribution_intent(
            &repos,
            &repos,
            &repos,
            &actor,
            RecordContributionIntentInput {
                vault_id,
                amount_minor: 25_000,
                currency: Currency::Usdc,
                source_type: ContributionSourceType::Usdc,
                external_reference: Some("stellar:intent-1".to_owned()),
                idempotency_key: "idem-1".to_owned(),
            },
        )
        .await
        .unwrap();

        let second = record_contribution_intent(
            &repos,
            &repos,
            &repos,
            &actor,
            RecordContributionIntentInput {
                vault_id,
                amount_minor: 25_000,
                currency: Currency::Usdc,
                source_type: ContributionSourceType::Usdc,
                external_reference: Some("stellar:intent-1".to_owned()),
                idempotency_key: "idem-1".to_owned(),
            },
        )
        .await
        .unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(repos.state.lock().await.contributions.len(), 1);
        assert_eq!(repos.state.lock().await.audits.len(), 1);
    }

    #[tokio::test]
    async fn idempotency_key_conflicts_when_payload_changes() {
        let repos = seeded_repos().await;
        let actor = contributor_user();
        let vault_id = seeded_vault_id();

        let _ = record_contribution_intent(
            &repos,
            &repos,
            &repos,
            &actor,
            RecordContributionIntentInput {
                vault_id,
                amount_minor: 25_000,
                currency: Currency::Usdc,
                source_type: ContributionSourceType::Usdc,
                external_reference: None,
                idempotency_key: "idem-2".to_owned(),
            },
        )
        .await
        .unwrap();

        let error = record_contribution_intent(
            &repos,
            &repos,
            &repos,
            &actor,
            RecordContributionIntentInput {
                vault_id,
                amount_minor: 30_000,
                currency: Currency::Usdc,
                source_type: ContributionSourceType::Usdc,
                external_reference: None,
                idempotency_key: "idem-2".to_owned(),
            },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, ContributionError::Conflict(_)));
    }

    #[tokio::test]
    async fn confirm_is_idempotent_and_updates_ledger_once() {
        let repos = seeded_repos().await;
        let actor = contributor_user();
        let admin = platform_admin_user();
        let vault_id = seeded_vault_id();

        let contribution = record_contribution_intent(
            &repos,
            &repos,
            &repos,
            &actor,
            RecordContributionIntentInput {
                vault_id,
                amount_minor: 25_000,
                currency: Currency::Usdc,
                source_type: ContributionSourceType::Usdc,
                external_reference: None,
                idempotency_key: "idem-3".to_owned(),
            },
        )
        .await
        .unwrap();

        let first = confirm_contribution(
            &repos,
            &repos,
            &admin,
            SettlementInput {
                contribution_id: contribution.id,
                external_reference: Some("stellar:tx-1".to_owned()),
            },
        )
        .await
        .unwrap();

        let second = confirm_contribution(
            &repos,
            &repos,
            &admin,
            SettlementInput {
                contribution_id: contribution.id,
                external_reference: Some("stellar:tx-1".to_owned()),
            },
        )
        .await
        .unwrap();

        let state = repos.state.lock().await;
        assert_eq!(first.status, ContributionStatus::Confirmed);
        assert_eq!(second.status, ContributionStatus::Confirmed);
        assert_eq!(state.ledger_entries.len(), 1);
        assert_eq!(state.vaults.get(&vault_id).unwrap().total_contributed_minor, 25_000);
    }

    #[tokio::test]
    async fn reverse_requires_confirmed_contribution() {
        let repos = seeded_repos().await;
        let admin = platform_admin_user();
        let contribution = seeded_pending_contribution(&repos).await;

        let error = reverse_contribution(
            &repos,
            &repos,
            &admin,
            SettlementInput { contribution_id: contribution.id, external_reference: None },
        )
        .await
        .unwrap_err();

        assert!(matches!(error, ContributionError::Conflict(_)));
    }

    #[tokio::test]
    async fn list_vault_contributions_requires_access() {
        let repos = seeded_repos().await;
        let stranger = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            role: UserRole::Contributor,
            session_id: Uuid::new_v4(),
        };

        let error = list_vault_contributions(&repos, &repos, &stranger, seeded_vault_id())
            .await
            .unwrap_err();

        assert!(matches!(error, ContributionError::Forbidden));
    }

    async fn seeded_repos() -> FakeRepos {
        let repos = FakeRepos::default();
        let vault_id = seeded_vault_id();
        let owner_id = seeded_parent_id();
        let mut state = repos.state.lock().await;
        state.vaults.insert(
            vault_id,
            SavingsVault {
                id: vault_id,
                plan_id: Uuid::new_v4(),
                owner_user_id: owner_id,
                currency: "USDC".to_owned(),
                status: VaultStatus::Active,
                total_contributed_minor: 0,
                total_locked_minor: 0,
                total_disbursed_minor: 0,
                external_wallet_account_id: None,
                external_contract_ref: None,
                version: 0,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );
        state.contributors.insert(
            vault_id,
            vec![VaultContributor {
                id: Uuid::new_v4(),
                vault_id,
                contributor_user_id: seeded_contributor_id(),
                role_label: "contributor".to_owned(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
        );
        drop(state);
        repos
    }

    async fn seeded_pending_contribution(repos: &FakeRepos) -> Contribution {
        record_contribution_intent(
            repos,
            repos,
            repos,
            &contributor_user(),
            RecordContributionIntentInput {
                vault_id: seeded_vault_id(),
                amount_minor: 25_000,
                currency: Currency::Usdc,
                source_type: ContributionSourceType::Usdc,
                external_reference: None,
                idempotency_key: "idem-pending".to_owned(),
            },
        )
        .await
        .unwrap()
    }

    fn contributor_user() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: seeded_contributor_id(),
            role: UserRole::Contributor,
            session_id: Uuid::new_v4(),
        }
    }

    fn platform_admin_user() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            role: UserRole::PlatformAdmin,
            session_id: Uuid::new_v4(),
        }
    }

    fn seeded_vault_id() -> Uuid {
        Uuid::from_u128(1)
    }

    fn seeded_parent_id() -> Uuid {
        Uuid::from_u128(2)
    }

    fn seeded_contributor_id() -> Uuid {
        Uuid::from_u128(3)
    }
}
