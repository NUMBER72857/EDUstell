use std::{fmt, str::FromStr};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::money::Money;
use uuid::Uuid;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = &'static str;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(concat!("invalid ", stringify!($name))),
                }
            }
        }
    };
}

string_enum!(RecordStatus {
    Active => "active",
    Inactive => "inactive",
    Suspended => "suspended"
});
string_enum!(PlanStatus {
    Draft => "draft",
    Active => "active",
    Paused => "paused",
    Completed => "completed",
    Cancelled => "cancelled"
});
string_enum!(VaultStatus {
    Active => "active",
    Locked => "locked",
    Closed => "closed"
});
string_enum!(MilestoneStatus {
    Planned => "planned",
    Funded => "funded",
    PartiallyPaid => "partially_paid",
    Paid => "paid",
    Cancelled => "cancelled"
});
string_enum!(PayoutType {
    Tuition => "tuition",
    ExamFee => "exam_fee",
    Deposit => "deposit",
    Supplies => "supplies"
});
string_enum!(SchoolVerificationStatus {
    Pending => "pending",
    Verified => "verified",
    Rejected => "rejected"
});
string_enum!(SchoolPayoutMethod {
    Stellar => "stellar",
    BankTransfer => "bank_transfer",
    MobileMoney => "mobile_money",
    FiatOfframp => "fiat_offramp",
    Manual => "manual"
});
string_enum!(ContributionStatus {
    Pending => "pending",
    Confirmed => "confirmed",
    Failed => "failed",
    Reversed => "reversed"
});
string_enum!(ContributionSourceType {
    Fiat => "fiat",
    Usdc => "usdc",
    Manual => "manual"
});
string_enum!(PayoutStatus {
    Pending => "pending",
    UnderReview => "under_review",
    Approved => "approved",
    Rejected => "rejected",
    Processing => "processing",
    Completed => "completed",
    Failed => "failed"
});
string_enum!(KycStatus {
    Pending => "pending",
    UnderReview => "under_review",
    Approved => "approved",
    Rejected => "rejected"
});
string_enum!(ScholarshipPoolStatus {
    Open => "open",
    Closed => "closed"
});
string_enum!(ScholarshipApplicationStatus {
    Submitted => "submitted",
    UnderReview => "under_review",
    Approved => "approved",
    Rejected => "rejected"
});
string_enum!(ScholarshipAwardStatus {
    Approved => "approved",
    Rejected => "rejected",
    Disbursed => "disbursed",
    Revoked => "revoked"
});
string_enum!(AchievementCredentialType {
    ScholarshipRecipient => "scholarship_recipient",
    FeeFullyFunded => "fee_fully_funded",
    AcademicExcellence => "academic_excellence",
    AttendanceRecognition => "attendance_recognition"
});
string_enum!(AchievementCredentialStatus {
    Issued => "issued",
    Revoked => "revoked"
});
string_enum!(DonorContributionStatus {
    Confirmed => "confirmed",
    Reversed => "reversed"
});
string_enum!(NotificationStatus {
    Pending => "pending",
    Sent => "sent",
    Read => "read",
    Failed => "failed"
});
string_enum!(NotificationType {
    ContributionReceived => "contribution_received",
    MilestoneDueSoon => "milestone_due_soon",
    MilestoneUnderfunded => "milestone_underfunded",
    PayoutApproved => "payout_approved",
    PayoutCompleted => "payout_completed",
    ScholarshipAwarded => "scholarship_awarded",
    KycActionRequired => "kyc_action_required"
});
string_enum!(ExternalReferenceEntityType {
    WalletAccount => "wallet_account",
    SavingsVault => "savings_vault",
    Contribution => "contribution",
    PayoutRequest => "payout_request"
});
string_enum!(ExternalReferenceKind {
    StellarAccountId => "stellar_account_id",
    SorobanContractId => "soroban_contract_id",
    TransactionHash => "transaction_hash"
});
string_enum!(BlockchainTransactionStatus {
    Pending => "pending",
    Submitted => "submitted",
    Confirmed => "confirmed",
    Failed => "failed",
    RetryScheduled => "retry_scheduled"
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildProfile {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub full_name: String,
    pub date_of_birth: Option<NaiveDate>,
    pub education_level: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsPlan {
    pub id: Uuid,
    pub child_profile_id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub target_amount: Money,
    pub status: PlanStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsVault {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub owner_user_id: Uuid,
    pub currency: String,
    pub status: VaultStatus,
    pub total_contributed_minor: i64,
    pub total_locked_minor: i64,
    pub total_disbursed_minor: i64,
    pub external_wallet_account_id: Option<Uuid>,
    pub external_contract_ref: Option<String>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultContributor {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub contributor_user_id: Uuid,
    pub role_label: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub due_date: NaiveDate,
    pub target_amount: Money,
    pub funded_amount: Money,
    pub payout_type: PayoutType,
    pub status: MilestoneStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contribution {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub contributor_user_id: Uuid,
    pub amount: Money,
    pub status: ContributionStatus,
    pub source_type: ContributionSourceType,
    pub external_reference: Option<String>,
    pub idempotency_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct School {
    pub id: Uuid,
    pub legal_name: String,
    pub display_name: String,
    pub country: String,
    pub payout_method: SchoolPayoutMethod,
    pub payout_reference: String,
    pub verification_status: SchoolVerificationStatus,
    pub verified_by: Option<Uuid>,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutRequest {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub milestone_id: Uuid,
    pub school_id: Uuid,
    pub requested_by: Uuid,
    pub amount: Money,
    pub idempotency_key: Option<String>,
    pub status: PayoutStatus,
    pub review_notes: Option<String>,
    pub external_payout_reference: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KycProfile {
    pub id: Uuid,
    pub user_id: Uuid,
    pub status: KycStatus,
    pub provider_reference: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarshipPool {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: ScholarshipPoolStatus,
    pub available_funds: Money,
    pub geography_restriction: Option<String>,
    pub education_level_restriction: Option<String>,
    pub school_id_restriction: Option<Uuid>,
    pub category_restriction: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarshipApplication {
    pub id: Uuid,
    pub pool_id: Uuid,
    pub applicant_user_id: Uuid,
    pub child_profile_id: Uuid,
    pub student_country: Option<String>,
    pub education_level: Option<String>,
    pub school_id: Option<Uuid>,
    pub category: Option<String>,
    pub status: ScholarshipApplicationStatus,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarshipAward {
    pub id: Uuid,
    pub application_id: Uuid,
    pub decided_by: Uuid,
    pub amount: Money,
    pub status: ScholarshipAwardStatus,
    pub decision_notes: Option<String>,
    pub linked_payout_request_id: Option<Uuid>,
    pub linked_vault_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementCredential {
    pub id: Uuid,
    pub credential_ref: Uuid,
    pub child_profile_id: Uuid,
    pub recipient_user_id: Option<Uuid>,
    pub school_id: Option<Uuid>,
    pub achievement_type: AchievementCredentialType,
    pub status: AchievementCredentialStatus,
    pub title: String,
    pub description: Option<String>,
    pub achievement_date: NaiveDate,
    pub issued_by_user_id: Uuid,
    pub issued_by_role: String,
    pub issuance_notes: Option<String>,
    pub evidence_uri: Option<String>,
    pub attestation_hash: String,
    pub attestation_method: String,
    pub attestation_anchor: Option<String>,
    pub attestation_anchor_network: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorContribution {
    pub id: Uuid,
    pub pool_id: Uuid,
    pub donor_user_id: Uuid,
    pub amount: Money,
    pub status: DonorContributionStatus,
    pub external_reference: Option<String>,
    pub idempotency_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub action: String,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub notification_type: NotificationType,
    pub title: String,
    pub body: String,
    pub metadata: Value,
    pub status: NotificationStatus,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationPreference {
    pub id: Uuid,
    pub user_id: Uuid,
    pub notification_type: NotificationType,
    pub in_app_enabled: bool,
    pub email_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAccount {
    pub id: Uuid,
    pub user_id: Uuid,
    pub network: String,
    pub address: String,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalReference {
    pub id: Uuid,
    pub entity_type: ExternalReferenceEntityType,
    pub entity_id: Uuid,
    pub reference_kind: ExternalReferenceKind,
    pub value: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainTransactionRecord {
    pub id: Uuid,
    pub entity_type: ExternalReferenceEntityType,
    pub entity_id: Uuid,
    pub operation_kind: String,
    pub idempotency_key: String,
    pub status: BlockchainTransactionStatus,
    pub tx_hash: Option<String>,
    pub attempt_count: i32,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
