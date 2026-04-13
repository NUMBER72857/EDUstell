use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::money::Money;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockchainNetwork {
    Stellar,
    Soroban,
}

impl BlockchainNetwork {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stellar => "stellar",
            Self::Soroban => "soroban",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletCustodyModel {
    Custodial,
    NonCustodial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SigningBoundary {
    PlatformCustodied { key_reference: String },
    UserControlled { account_id: String },
    PreSignedEnvelope { envelope_xdr: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletProvisioningRequest {
    pub wallet_id: Uuid,
    pub owner_user_id: Uuid,
    pub network: BlockchainNetwork,
    pub custody_model: WalletCustodyModel,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionedWallet {
    pub wallet_id: Uuid,
    pub network: BlockchainNetwork,
    pub custody_model: WalletCustodyModel,
    pub stellar_account_id: String,
    pub signing_boundary: SigningBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsdcBalanceLookupRequest {
    pub stellar_account_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsdcBalance {
    pub stellar_account_id: String,
    pub balance_minor: i64,
    pub decimals: u32,
    pub last_ledger: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContractArgument {
    Address(String),
    Integer(i64),
    Unsigned(u64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VaultContractCallRequest {
    pub operation_id: String,
    pub contract_id: String,
    pub method: String,
    pub args: Vec<ContractArgument>,
    pub amount: Option<Money>,
    pub signing_boundary: SigningBoundary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreparedTransaction {
    pub operation_id: String,
    pub network: BlockchainNetwork,
    pub source_account_id: String,
    pub contract_id: Option<String>,
    pub unsigned_envelope_xdr: String,
    pub signing_boundary: SigningBoundary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionSubmissionRequest {
    pub operation_id: String,
    pub prepared_transaction: PreparedTransaction,
    pub authorization: SigningBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    Pending,
    Succeeded,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmittedTransaction {
    pub operation_id: String,
    pub tx_hash: String,
    pub state: TransactionState,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionStatusRequest {
    pub tx_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionStatus {
    pub tx_hash: String,
    pub state: TransactionState,
    pub ledger: Option<u64>,
    pub retryable: bool,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventCursor {
    pub stream: String,
    pub position: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventIngestionRequest {
    pub cursor: Option<EventCursor>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockchainEvent {
    pub event_id: String,
    pub network: BlockchainNetwork,
    pub contract_id: Option<String>,
    pub tx_hash: Option<String>,
    pub topic: String,
    pub payload: String,
    pub ledger: Option<u64>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventBatch {
    pub events: Vec<BlockchainEvent>,
    pub next_cursor: Option<EventCursor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockchainErrorCode {
    Validation,
    Unauthorized,
    NotFound,
    Conflict,
    InsufficientFunds,
    SignatureRequired,
    SubmissionRejected,
    Timeout,
    RateLimited,
    Unavailable,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockchainError {
    pub code: BlockchainErrorCode,
    pub message: String,
    pub retryable: bool,
    pub provider: &'static str,
    pub external_code: Option<String>,
}

impl BlockchainError {
    pub fn new(
        code: BlockchainErrorCode,
        message: impl Into<String>,
        retryable: bool,
        provider: &'static str,
    ) -> Self {
        Self { code, message: message.into(), retryable, provider, external_code: None }
    }

    pub fn with_external_code(mut self, code: impl Into<String>) -> Self {
        self.external_code = Some(code.into());
        self
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(BlockchainErrorCode::Unavailable, message, true, "stellar_soroban_stub")
    }
}

impl fmt::Display for BlockchainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?})", self.message, self.code)
    }
}

impl std::error::Error for BlockchainError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StellarProviderError {
    BadRequest,
    Unauthorized,
    AccountMissing,
    ContractMissing,
    DuplicateTransaction,
    SimulationFailed,
    InsufficientFunds,
    Timeout,
    RateLimited,
    RpcUnavailable,
    Unknown,
}

pub fn map_stellar_provider_error(
    provider_error: StellarProviderError,
    detail: impl Into<String>,
) -> BlockchainError {
    let detail = detail.into();

    match provider_error {
        StellarProviderError::BadRequest => {
            BlockchainError::new(BlockchainErrorCode::Validation, detail, false, "stellar")
        }
        StellarProviderError::Unauthorized => {
            BlockchainError::new(BlockchainErrorCode::Unauthorized, detail, false, "stellar")
        }
        StellarProviderError::AccountMissing | StellarProviderError::ContractMissing => {
            BlockchainError::new(BlockchainErrorCode::NotFound, detail, false, "stellar")
        }
        StellarProviderError::DuplicateTransaction => {
            BlockchainError::new(BlockchainErrorCode::Conflict, detail, false, "stellar")
        }
        StellarProviderError::SimulationFailed => {
            BlockchainError::new(BlockchainErrorCode::SubmissionRejected, detail, false, "soroban")
        }
        StellarProviderError::InsufficientFunds => {
            BlockchainError::new(BlockchainErrorCode::InsufficientFunds, detail, false, "stellar")
        }
        StellarProviderError::Timeout => {
            BlockchainError::new(BlockchainErrorCode::Timeout, detail, true, "stellar")
        }
        StellarProviderError::RateLimited => {
            BlockchainError::new(BlockchainErrorCode::RateLimited, detail, true, "stellar")
        }
        StellarProviderError::RpcUnavailable => {
            BlockchainError::new(BlockchainErrorCode::Unavailable, detail, true, "soroban")
        }
        StellarProviderError::Unknown => {
            BlockchainError::new(BlockchainErrorCode::Internal, detail, true, "stellar")
        }
    }
}

#[async_trait]
pub trait WalletProvisioner: Send + Sync {
    async fn provision_wallet(
        &self,
        request: WalletProvisioningRequest,
    ) -> Result<ProvisionedWallet, BlockchainError>;
}

#[async_trait]
pub trait UsdcBalanceLookup: Send + Sync {
    async fn lookup_usdc_balance(
        &self,
        request: UsdcBalanceLookupRequest,
    ) -> Result<UsdcBalance, BlockchainError>;
}

#[async_trait]
pub trait VaultContractClient: Send + Sync {
    async fn prepare_vault_call(
        &self,
        request: VaultContractCallRequest,
    ) -> Result<PreparedTransaction, BlockchainError>;
}

#[async_trait]
pub trait TransactionSubmitter: Send + Sync {
    async fn submit_transaction(
        &self,
        request: TransactionSubmissionRequest,
    ) -> Result<SubmittedTransaction, BlockchainError>;
}

#[async_trait]
pub trait TransactionStatusLookup: Send + Sync {
    async fn lookup_transaction_status(
        &self,
        request: TransactionStatusRequest,
    ) -> Result<TransactionStatus, BlockchainError>;
}

#[async_trait]
pub trait EventIngestor: Send + Sync {
    async fn ingest_events(
        &self,
        request: EventIngestionRequest,
    ) -> Result<EventBatch, BlockchainError>;
}

#[derive(Debug, Clone)]
pub struct StellarSorobanAdapter {
    pub network: BlockchainNetwork,
    pub horizon_url: Option<String>,
    pub rpc_url: Option<String>,
    pub usdc_issuer: Option<String>,
    pub stub_mode: bool,
}

impl Default for StellarSorobanAdapter {
    fn default() -> Self {
        Self {
            network: BlockchainNetwork::Soroban,
            horizon_url: None,
            rpc_url: None,
            usdc_issuer: None,
            stub_mode: true,
        }
    }
}

impl StellarSorobanAdapter {
    fn ensure_stub_mode(&self) -> Result<(), BlockchainError> {
        if self.stub_mode {
            Ok(())
        } else {
            Err(BlockchainError::unavailable(
                "live Stellar/Soroban RPC calls are not implemented yet",
            ))
        }
    }
}

#[async_trait]
impl WalletProvisioner for StellarSorobanAdapter {
    async fn provision_wallet(
        &self,
        request: WalletProvisioningRequest,
    ) -> Result<ProvisionedWallet, BlockchainError> {
        self.ensure_stub_mode()?;

        let account_id = format!("G{}", request.wallet_id.simple());
        let signing_boundary = match request.custody_model {
            WalletCustodyModel::Custodial => SigningBoundary::PlatformCustodied {
                key_reference: format!("stellar-key:{}", request.wallet_id.simple()),
            },
            WalletCustodyModel::NonCustodial => {
                SigningBoundary::UserControlled { account_id: account_id.clone() }
            }
        };

        Ok(ProvisionedWallet {
            wallet_id: request.wallet_id,
            network: request.network,
            custody_model: request.custody_model,
            stellar_account_id: account_id,
            signing_boundary,
        })
    }
}

#[async_trait]
impl UsdcBalanceLookup for StellarSorobanAdapter {
    async fn lookup_usdc_balance(
        &self,
        request: UsdcBalanceLookupRequest,
    ) -> Result<UsdcBalance, BlockchainError> {
        self.ensure_stub_mode()?;

        Ok(UsdcBalance {
            stellar_account_id: request.stellar_account_id,
            balance_minor: 0,
            decimals: 7,
            last_ledger: None,
        })
    }
}

#[async_trait]
impl VaultContractClient for StellarSorobanAdapter {
    async fn prepare_vault_call(
        &self,
        request: VaultContractCallRequest,
    ) -> Result<PreparedTransaction, BlockchainError> {
        self.ensure_stub_mode()?;

        let source_account_id = match &request.signing_boundary {
            SigningBoundary::PlatformCustodied { key_reference } => key_reference.clone(),
            SigningBoundary::UserControlled { account_id } => account_id.clone(),
            SigningBoundary::PreSignedEnvelope { .. } => "presigned-envelope".to_owned(),
        };

        Ok(PreparedTransaction {
            operation_id: request.operation_id,
            network: self.network,
            source_account_id,
            contract_id: Some(request.contract_id),
            unsigned_envelope_xdr: "AAAA-stubbed-xdr".to_owned(),
            signing_boundary: request.signing_boundary,
        })
    }
}

#[async_trait]
impl TransactionSubmitter for StellarSorobanAdapter {
    async fn submit_transaction(
        &self,
        request: TransactionSubmissionRequest,
    ) -> Result<SubmittedTransaction, BlockchainError> {
        self.ensure_stub_mode()?;

        Ok(SubmittedTransaction {
            operation_id: request.operation_id.clone(),
            tx_hash: format!("stub-tx-{}", sanitize_identifier(&request.operation_id)),
            state: TransactionState::Pending,
            submitted_at: Utc::now(),
        })
    }
}

#[async_trait]
impl TransactionStatusLookup for StellarSorobanAdapter {
    async fn lookup_transaction_status(
        &self,
        request: TransactionStatusRequest,
    ) -> Result<TransactionStatus, BlockchainError> {
        self.ensure_stub_mode()?;

        Ok(TransactionStatus {
            tx_hash: request.tx_hash,
            state: TransactionState::Pending,
            ledger: None,
            retryable: true,
            error_code: None,
            error_message: None,
            observed_at: Utc::now(),
        })
    }
}

#[async_trait]
impl EventIngestor for StellarSorobanAdapter {
    async fn ingest_events(
        &self,
        request: EventIngestionRequest,
    ) -> Result<EventBatch, BlockchainError> {
        self.ensure_stub_mode()?;

        let next_cursor = request.cursor.map(|cursor| EventCursor {
            stream: cursor.stream,
            position: format!("{}:noop", cursor.position),
        });

        Ok(EventBatch { events: Vec::new(), next_cursor })
    }
}

fn sanitize_identifier(value: &str) -> String {
    value.chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' }).collect()
}
