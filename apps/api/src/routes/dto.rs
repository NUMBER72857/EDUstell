use chrono::{DateTime, Utc};
use domain::{
    auth::PublicUser,
    persistence::{
        AchievementCredential, Contribution, DonorContribution, Notification,
        NotificationPreference, PayoutRequest, ScholarshipApplication, ScholarshipAward,
        ScholarshipPool, School,
    },
};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize, Clone)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub email_verified: bool,
    pub mfa_enabled: bool,
}

impl From<PublicUser> for UserDto {
    fn from(value: PublicUser) -> Self {
        Self {
            id: value.id,
            email: value.email,
            role: value.role.as_str().to_owned(),
            email_verified: value.email_verified,
            mfa_enabled: value.mfa_enabled,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct MoneyDto {
    pub amount_minor: i64,
    pub currency: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ContributionDto {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub contributor_user_id: Uuid,
    pub amount: MoneyDto,
    pub status: String,
    pub source_type: String,
    pub external_reference_present: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Contribution> for ContributionDto {
    fn from(value: Contribution) -> Self {
        Self {
            id: value.id,
            vault_id: value.vault_id,
            contributor_user_id: value.contributor_user_id,
            amount: MoneyDto {
                amount_minor: value.amount.amount_minor,
                currency: value.amount.currency.as_str().to_owned(),
            },
            status: value.status.as_str().to_owned(),
            source_type: value.source_type.as_str().to_owned(),
            external_reference_present: value.external_reference.is_some(),
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct PayoutDto {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub milestone_id: Uuid,
    pub school_id: Uuid,
    pub requested_by: Uuid,
    pub amount: MoneyDto,
    pub idempotency_key_present: bool,
    pub status: String,
    pub review_notes_present: bool,
    pub external_payout_reference_present: bool,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<PayoutRequest> for PayoutDto {
    fn from(value: PayoutRequest) -> Self {
        Self {
            id: value.id,
            vault_id: value.vault_id,
            milestone_id: value.milestone_id,
            school_id: value.school_id,
            requested_by: value.requested_by,
            amount: MoneyDto {
                amount_minor: value.amount.amount_minor,
                currency: value.amount.currency.as_str().to_owned(),
            },
            idempotency_key_present: value.idempotency_key.is_some(),
            status: value.status.as_str().to_owned(),
            review_notes_present: value.review_notes.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
            external_payout_reference_present: value.external_payout_reference.is_some(),
            reviewed_by: value.reviewed_by,
            reviewed_at: value.reviewed_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct SchoolDto {
    pub id: Uuid,
    pub legal_name: String,
    pub display_name: String,
    pub country: String,
    pub payout_method: String,
    pub verification_status: String,
    pub verified_by: Option<Uuid>,
    pub verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<School> for SchoolDto {
    fn from(value: School) -> Self {
        Self {
            id: value.id,
            legal_name: value.legal_name,
            display_name: value.display_name,
            country: value.country,
            payout_method: value.payout_method.as_str().to_owned(),
            verification_status: value.verification_status.as_str().to_owned(),
            verified_by: value.verified_by,
            verified_at: value.verified_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct NotificationDto {
    pub id: Uuid,
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub metadata: serde_json::Value,
    pub status: String,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<Notification> for NotificationDto {
    fn from(value: Notification) -> Self {
        Self {
            id: value.id,
            notification_type: value.notification_type.as_str().to_owned(),
            title: value.title,
            body: value.body,
            metadata: value.metadata,
            status: value.status.as_str().to_owned(),
            read_at: value.read_at,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct NotificationPreferenceDto {
    pub id: Uuid,
    pub notification_type: String,
    pub in_app_enabled: bool,
    pub email_enabled: bool,
    pub updated_at: DateTime<Utc>,
}

impl From<NotificationPreference> for NotificationPreferenceDto {
    fn from(value: NotificationPreference) -> Self {
        Self {
            id: value.id,
            notification_type: value.notification_type.as_str().to_owned(),
            in_app_enabled: value.in_app_enabled,
            email_enabled: value.email_enabled,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ScholarshipPoolDto {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub available_funds: MoneyDto,
    pub geography_restriction: Option<String>,
    pub education_level_restriction: Option<String>,
    pub school_id_restriction: Option<Uuid>,
    pub category_restriction: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<ScholarshipPool> for ScholarshipPoolDto {
    fn from(value: ScholarshipPool) -> Self {
        Self {
            id: value.id,
            owner_user_id: value.owner_user_id,
            name: value.name,
            description: value.description,
            status: value.status.as_str().to_owned(),
            available_funds: MoneyDto {
                amount_minor: value.available_funds.amount_minor,
                currency: value.available_funds.currency.as_str().to_owned(),
            },
            geography_restriction: value.geography_restriction,
            education_level_restriction: value.education_level_restriction,
            school_id_restriction: value.school_id_restriction,
            category_restriction: value.category_restriction,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct AchievementCredentialDto {
    pub id: Uuid,
    pub credential_ref: Uuid,
    pub child_profile_id: Uuid,
    pub recipient_user_id: Option<Uuid>,
    pub school_id: Option<Uuid>,
    pub achievement_type: String,
    pub status: String,
    pub title: String,
    pub description: Option<String>,
    pub achievement_date: chrono::NaiveDate,
    pub issued_by_user_id: Uuid,
    pub issued_by_role: String,
    pub issuance_notes_present: bool,
    pub evidence_uri_present: bool,
    pub attestation_hash: String,
    pub attestation_method: String,
    pub attestation_anchor: Option<String>,
    pub attestation_anchor_network: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AchievementCredential> for AchievementCredentialDto {
    fn from(value: AchievementCredential) -> Self {
        Self {
            id: value.id,
            credential_ref: value.credential_ref,
            child_profile_id: value.child_profile_id,
            recipient_user_id: value.recipient_user_id,
            school_id: value.school_id,
            achievement_type: value.achievement_type.as_str().to_owned(),
            status: value.status.as_str().to_owned(),
            title: value.title,
            description: value.description,
            achievement_date: value.achievement_date,
            issued_by_user_id: value.issued_by_user_id,
            issued_by_role: value.issued_by_role,
            issuance_notes_present: value.issuance_notes.is_some(),
            evidence_uri_present: value.evidence_uri.is_some(),
            attestation_hash: value.attestation_hash,
            attestation_method: value.attestation_method,
            attestation_anchor: value.attestation_anchor,
            attestation_anchor_network: value.attestation_anchor_network,
            metadata: value.metadata,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ScholarshipApplicationDto {
    pub id: Uuid,
    pub pool_id: Uuid,
    pub applicant_user_id: Uuid,
    pub child_profile_id: Uuid,
    pub student_country: Option<String>,
    pub education_level: Option<String>,
    pub school_id: Option<Uuid>,
    pub category: Option<String>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<ScholarshipApplication> for ScholarshipApplicationDto {
    fn from(value: ScholarshipApplication) -> Self {
        Self {
            id: value.id,
            pool_id: value.pool_id,
            applicant_user_id: value.applicant_user_id,
            child_profile_id: value.child_profile_id,
            student_country: value.student_country,
            education_level: value.education_level,
            school_id: value.school_id,
            category: value.category,
            status: value.status.as_str().to_owned(),
            notes: value.notes,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ScholarshipAwardDto {
    pub id: Uuid,
    pub application_id: Uuid,
    pub decided_by: Uuid,
    pub amount: MoneyDto,
    pub status: String,
    pub decision_notes_present: bool,
    pub linked_payout_request_id: Option<Uuid>,
    pub linked_vault_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl From<ScholarshipAward> for ScholarshipAwardDto {
    fn from(value: ScholarshipAward) -> Self {
        Self {
            id: value.id,
            application_id: value.application_id,
            decided_by: value.decided_by,
            amount: MoneyDto {
                amount_minor: value.amount.amount_minor,
                currency: value.amount.currency.as_str().to_owned(),
            },
            status: value.status.as_str().to_owned(),
            decision_notes_present: value.decision_notes.as_ref().map(|v| !v.is_empty()).unwrap_or(false),
            linked_payout_request_id: value.linked_payout_request_id,
            linked_vault_id: value.linked_vault_id,
            created_at: value.created_at,
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct DonorContributionDto {
    pub id: Uuid,
    pub pool_id: Uuid,
    pub donor_user_id: Uuid,
    pub amount: MoneyDto,
    pub status: String,
    pub external_reference_present: bool,
    pub created_at: DateTime<Utc>,
}

impl From<DonorContribution> for DonorContributionDto {
    fn from(value: DonorContribution) -> Self {
        Self {
            id: value.id,
            pool_id: value.pool_id,
            donor_user_id: value.donor_user_id,
            amount: MoneyDto {
                amount_minor: value.amount.amount_minor,
                currency: value.amount.currency.as_str().to_owned(),
            },
            status: value.status.as_str().to_owned(),
            external_reference_present: value.external_reference.is_some(),
            created_at: value.created_at,
        }
    }
}
