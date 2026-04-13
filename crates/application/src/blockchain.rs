use blockchain::{
    BlockchainError, BlockchainErrorCode, ContractArgument, PreparedTransaction, ProvisionedWallet,
    SigningBoundary, SubmittedTransaction, TransactionState, TransactionStatusLookup,
    TransactionSubmissionRequest, TransactionSubmitter, VaultContractCallRequest,
    VaultContractClient, WalletCustodyModel, WalletProvisioner, WalletProvisioningRequest,
};
use chrono::Utc;
use domain::{
    auth::{AuthenticatedUser, UserRole},
    persistence::{
        BlockchainTransactionRecord, BlockchainTransactionStatus, Contribution, ExternalReference,
        ExternalReferenceEntityType, ExternalReferenceKind, SavingsVault, WalletAccount,
    },
};
use serde_json::json;
use uuid::Uuid;

use crate::{
    audit::{AuditEvent, AuditService},
    contributions::{ContributionError, SettlementInput, confirm_contribution, fail_contribution},
    repos::{
        AuditLogRepository, BlockchainTransactionRepository, ContributionRepository,
        ContributionWorkflowRepository, ExternalReferenceRepository, PersistenceError,
        VaultRepository, WalletAccountRepository,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum BlockchainFacadeError {
    #[error("validation: {0}")]
    Validation(String),
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("blockchain unavailable: {0}")]
    Unavailable(String),
    #[error("blockchain rejected request: {0}")]
    Rejected(String),
    #[error("repository error: {0}")]
    Repository(String),
}

impl From<PersistenceError> for BlockchainFacadeError {
    fn from(value: PersistenceError) -> Self {
        match value {
            PersistenceError::NotFound => Self::NotFound,
            PersistenceError::Conflict(message) => Self::Conflict(message),
            PersistenceError::Validation(message) => Self::Validation(message),
            PersistenceError::Repository(message) => Self::Repository(message),
        }
    }
}

impl From<BlockchainError> for BlockchainFacadeError {
    fn from(value: BlockchainError) -> Self {
        match value.code {
            BlockchainErrorCode::Validation => Self::Validation(value.message),
            BlockchainErrorCode::Unauthorized => Self::Forbidden,
            BlockchainErrorCode::NotFound => Self::NotFound,
            BlockchainErrorCode::Conflict => Self::Conflict(value.message),
            BlockchainErrorCode::SubmissionRejected
            | BlockchainErrorCode::InsufficientFunds
            | BlockchainErrorCode::SignatureRequired => Self::Rejected(value.message),
            BlockchainErrorCode::Timeout
            | BlockchainErrorCode::RateLimited
            | BlockchainErrorCode::Unavailable
            | BlockchainErrorCode::Internal => Self::Unavailable(value.message),
        }
    }
}

impl From<ContributionError> for BlockchainFacadeError {
    fn from(value: ContributionError) -> Self {
        match value {
            ContributionError::Validation(message) => Self::Validation(message),
            ContributionError::Forbidden => Self::Forbidden,
            ContributionError::NotFound => Self::NotFound,
            ContributionError::Conflict(message) => Self::Conflict(message),
            ContributionError::Repository(message) => Self::Repository(message),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProvisionUserWalletInput {
    pub owner_user_id: Uuid,
    pub label: Option<String>,
    pub custody_model: WalletCustodyModel,
}

#[derive(Debug, Clone)]
pub struct ProvisionUserWalletResult {
    pub wallet_account: WalletAccount,
    pub provisioned_wallet: ProvisionedWallet,
}

pub async fn provision_user_wallet<W, A, R, L>(
    provisioner: &W,
    wallets: &A,
    references: &R,
    audits: &L,
    input: ProvisionUserWalletInput,
) -> Result<ProvisionUserWalletResult, BlockchainFacadeError>
where
    W: WalletProvisioner,
    A: WalletAccountRepository,
    R: ExternalReferenceRepository,
    L: AuditLogRepository,
{
    let now = Utc::now();
    let wallet_account = WalletAccount {
        id: Uuid::new_v4(),
        user_id: input.owner_user_id,
        network: "stellar".to_owned(),
        address: "pending-provision".to_owned(),
        label: input.label.clone(),
        created_at: now,
        updated_at: now,
    };

    let provisioned = provisioner
        .provision_wallet(WalletProvisioningRequest {
            wallet_id: wallet_account.id,
            owner_user_id: input.owner_user_id,
            network: blockchain::BlockchainNetwork::Stellar,
            custody_model: input.custody_model,
            label: input.label,
        })
        .await?;

    let persisted_wallet = wallets
        .create(WalletAccount { address: provisioned.stellar_account_id.clone(), ..wallet_account })
        .await?;

    let reference = references
        .create(ExternalReference {
            id: Uuid::new_v4(),
            entity_type: ExternalReferenceEntityType::WalletAccount,
            entity_id: persisted_wallet.id,
            reference_kind: ExternalReferenceKind::StellarAccountId,
            value: provisioned.stellar_account_id.clone(),
            metadata: json!({
                "custody_model": match provisioned.custody_model {
                    WalletCustodyModel::Custodial => "custodial",
                    WalletCustodyModel::NonCustodial => "non_custodial",
                }
            }),
            created_at: now,
            updated_at: now,
        })
        .await?;
    AuditService::new(audits)
        .record(AuditEvent::blockchain_reference_attached(Some(input.owner_user_id), &reference))
        .await
        .map_err(|error| BlockchainFacadeError::Repository(error.to_string()))?;

    Ok(ProvisionUserWalletResult {
        wallet_account: persisted_wallet,
        provisioned_wallet: provisioned,
    })
}

#[derive(Debug, Clone)]
pub struct SubmitContributionOnchainInput {
    pub contribution_id: Uuid,
    pub idempotency_key: String,
    pub signing_boundary: SigningBoundary,
}

#[derive(Debug, Clone)]
pub struct ContributionOnchainSubmission {
    pub contribution: Contribution,
    pub processing_record: BlockchainTransactionRecord,
    pub prepared_transaction: PreparedTransaction,
    pub submitted_transaction: Option<SubmittedTransaction>,
}

pub async fn submit_contribution_onchain<V, C, T, R, X, S, A>(
    vaults: &V,
    contributions: &C,
    tx_repo: &T,
    references: &R,
    audits: &A,
    contract_client: &X,
    submitter: &S,
    input: SubmitContributionOnchainInput,
) -> Result<ContributionOnchainSubmission, BlockchainFacadeError>
where
    V: VaultRepository,
    C: ContributionRepository,
    T: BlockchainTransactionRepository,
    R: ExternalReferenceRepository,
    A: AuditLogRepository,
    X: VaultContractClient,
    S: TransactionSubmitter,
{
    let contribution = contributions
        .find_by_id(input.contribution_id)
        .await?
        .ok_or(BlockchainFacadeError::NotFound)?;
    let vault =
        vaults.find_by_id(contribution.vault_id).await?.ok_or(BlockchainFacadeError::NotFound)?;

    let contract_reference = find_contract_reference(references, &vault).await?;

    if let Some(existing) = tx_repo.find_by_idempotency_key(&input.idempotency_key).await? {
        let prepared = build_prepared_transaction(
            contract_client,
            &contribution,
            &contract_reference,
            input.signing_boundary.clone(),
            existing.idempotency_key.clone(),
        )
        .await?;

        return Ok(ContributionOnchainSubmission {
            contribution,
            processing_record: existing,
            prepared_transaction: prepared,
            submitted_transaction: None,
        });
    }

    let prepared = build_prepared_transaction(
        contract_client,
        &contribution,
        &contract_reference,
        input.signing_boundary.clone(),
        input.idempotency_key.clone(),
    )
    .await?;

    let submitted = submitter
        .submit_transaction(TransactionSubmissionRequest {
            operation_id: input.idempotency_key.clone(),
            prepared_transaction: prepared.clone(),
            authorization: input.signing_boundary,
        })
        .await?;

    let now = Utc::now();
    let record = tx_repo
        .create(BlockchainTransactionRecord {
            id: Uuid::new_v4(),
            entity_type: ExternalReferenceEntityType::Contribution,
            entity_id: contribution.id,
            operation_kind: "vault_contribution".to_owned(),
            idempotency_key: input.idempotency_key,
            status: BlockchainTransactionStatus::Submitted,
            tx_hash: Some(submitted.tx_hash.clone()),
            attempt_count: 1,
            last_error_code: None,
            last_error_message: None,
            next_retry_at: None,
            metadata: json!({
                "vault_id": vault.id,
                "contract_id": contract_reference.value,
                "amount_minor": contribution.amount.amount_minor,
                "currency": contribution.amount.currency.as_str(),
            }),
            created_at: now,
            updated_at: now,
        })
        .await?;

    store_tx_hash_reference(
        references,
        audits,
        Some(contribution.contributor_user_id),
        contribution.id,
        &submitted.tx_hash,
        now,
    )
    .await?;

    Ok(ContributionOnchainSubmission {
        contribution,
        processing_record: record,
        prepared_transaction: prepared,
        submitted_transaction: Some(submitted),
    })
}

pub async fn reconcile_contribution_transaction<C, W, T, S>(
    contributions: &C,
    workflow: &W,
    tx_repo: &T,
    status_lookup: &S,
    system_actor_user_id: Uuid,
    contribution_id: Uuid,
) -> Result<Option<Contribution>, BlockchainFacadeError>
where
    C: ContributionRepository,
    W: ContributionWorkflowRepository,
    T: BlockchainTransactionRepository,
    S: TransactionStatusLookup,
{
    let Some(mut record) = tx_repo
        .find_by_entity(
            ExternalReferenceEntityType::Contribution.as_str(),
            contribution_id,
            "vault_contribution",
        )
        .await?
    else {
        return Ok(None);
    };

    let Some(tx_hash) = record.tx_hash.clone() else {
        return Err(BlockchainFacadeError::Conflict(
            "blockchain transaction record missing tx hash".to_owned(),
        ));
    };

    let status = status_lookup
        .lookup_transaction_status(blockchain::TransactionStatusRequest { tx_hash })
        .await?;
    let actor = system_admin_actor(system_actor_user_id);

    let contribution = match status.state {
        TransactionState::Succeeded => {
            record.status = BlockchainTransactionStatus::Confirmed;
            record.last_error_code = None;
            record.last_error_message = None;
            record.next_retry_at = None;
            let contribution = confirm_contribution(
                contributions,
                workflow,
                &actor,
                SettlementInput { contribution_id, external_reference: record.tx_hash.clone() },
            )
            .await?;
            Some(contribution)
        }
        TransactionState::Failed => {
            record.status = if status.retryable {
                BlockchainTransactionStatus::RetryScheduled
            } else {
                BlockchainTransactionStatus::Failed
            };
            record.last_error_code = status.error_code;
            record.last_error_message = status.error_message;
            record.next_retry_at = status.retryable.then_some(Utc::now());
            if !status.retryable {
                let contribution = fail_contribution(
                    contributions,
                    workflow,
                    &actor,
                    SettlementInput { contribution_id, external_reference: record.tx_hash.clone() },
                )
                .await?;
                Some(contribution)
            } else {
                None
            }
        }
        TransactionState::Pending | TransactionState::Unknown => {
            record.status = BlockchainTransactionStatus::Submitted;
            None
        }
    };

    record.updated_at = Utc::now();
    tx_repo.update(record).await?;

    Ok(contribution)
}

async fn build_prepared_transaction<X>(
    contract_client: &X,
    contribution: &Contribution,
    contract_reference: &ExternalReference,
    signing_boundary: SigningBoundary,
    operation_id: String,
) -> Result<PreparedTransaction, BlockchainFacadeError>
where
    X: VaultContractClient,
{
    contract_client
        .prepare_vault_call(VaultContractCallRequest {
            operation_id,
            contract_id: contract_reference.value.clone(),
            method: "fund_vault".to_owned(),
            args: vec![
                ContractArgument::Text(contribution.vault_id.to_string()),
                ContractArgument::Integer(contribution.amount.amount_minor),
                ContractArgument::Text(contribution.amount.currency.as_str().to_owned()),
            ],
            amount: Some(contribution.amount.clone()),
            signing_boundary,
        })
        .await
        .map_err(Into::into)
}

async fn find_contract_reference<R>(
    references: &R,
    vault: &SavingsVault,
) -> Result<ExternalReference, BlockchainFacadeError>
where
    R: ExternalReferenceRepository,
{
    if let Some(reference) = references
        .find_one(
            ExternalReferenceEntityType::SavingsVault.as_str(),
            vault.id,
            ExternalReferenceKind::SorobanContractId.as_str(),
        )
        .await?
    {
        return Ok(reference);
    }

    if let Some(existing) = &vault.external_contract_ref {
        let now = Utc::now();
        return Ok(ExternalReference {
            id: Uuid::new_v4(),
            entity_type: ExternalReferenceEntityType::SavingsVault,
            entity_id: vault.id,
            reference_kind: ExternalReferenceKind::SorobanContractId,
            value: existing.clone(),
            metadata: json!({ "source": "legacy_vault_column" }),
            created_at: now,
            updated_at: now,
        });
    }

    Err(BlockchainFacadeError::NotFound)
}

async fn store_tx_hash_reference<R, A>(
    references: &R,
    audits: &A,
    actor_user_id: Option<Uuid>,
    contribution_id: Uuid,
    tx_hash: &str,
    now: chrono::DateTime<Utc>,
) -> Result<(), BlockchainFacadeError>
where
    R: ExternalReferenceRepository,
    A: AuditLogRepository,
{
    let reference = references
        .create(ExternalReference {
            id: Uuid::new_v4(),
            entity_type: ExternalReferenceEntityType::Contribution,
            entity_id: contribution_id,
            reference_kind: ExternalReferenceKind::TransactionHash,
            value: tx_hash.to_owned(),
            metadata: json!({}),
            created_at: now,
            updated_at: now,
        })
        .await?;
    AuditService::new(audits)
        .record(AuditEvent::blockchain_reference_attached(actor_user_id, &reference))
        .await
        .map_err(|error| BlockchainFacadeError::Repository(error.to_string()))?;

    Ok(())
}

fn system_admin_actor(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser { user_id, role: UserRole::PlatformAdmin, session_id: Uuid::nil() }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use super::*;

    #[derive(Default, Clone)]
    struct FakeReferences {
        items: Arc<Mutex<HashMap<Uuid, ExternalReference>>>,
    }

    #[async_trait]
    impl ExternalReferenceRepository for FakeReferences {
        async fn create(
            &self,
            reference: ExternalReference,
        ) -> Result<ExternalReference, PersistenceError> {
            self.items.lock().await.insert(reference.id, reference.clone());
            Ok(reference)
        }

        async fn find_by_entity(
            &self,
            entity_type: &str,
            entity_id: Uuid,
        ) -> Result<Vec<ExternalReference>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .await
                .values()
                .filter(|item| {
                    item.entity_type.as_str() == entity_type && item.entity_id == entity_id
                })
                .cloned()
                .collect())
        }

        async fn find_one(
            &self,
            entity_type: &str,
            entity_id: Uuid,
            reference_kind: &str,
        ) -> Result<Option<ExternalReference>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .await
                .values()
                .find(|item| {
                    item.entity_type.as_str() == entity_type
                        && item.entity_id == entity_id
                        && item.reference_kind.as_str() == reference_kind
                })
                .cloned())
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
    async fn blockchain_reference_audit_uses_preview_not_full_value() {
        let references = FakeReferences::default();
        let audits = FakeAudits::default();
        let actor_id = Uuid::new_v4();
        let contribution_id = Uuid::new_v4();

        store_tx_hash_reference(
            &references,
            &audits,
            Some(actor_id),
            contribution_id,
            "0123456789abcdef0123456789abcdef",
            Utc::now(),
        )
        .await
        .unwrap();

        let logged = audits.items.lock().await;
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].action, "blockchain.reference_attached");
        assert_eq!(logged[0].metadata["reference_preview"], "012345...cdef");
    }
}
