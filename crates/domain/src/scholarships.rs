use std::fmt;

use uuid::Uuid;

use crate::auth::UserRole;
use crate::persistence::{
    ScholarshipApplicationStatus, ScholarshipAwardStatus, ScholarshipPool, ScholarshipPoolStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwardTransition {
    Approve,
    Reject,
    Disburse,
    Revoke,
}

impl AwardTransition {
    pub fn target_status(self) -> ScholarshipAwardStatus {
        match self {
            Self::Approve => ScholarshipAwardStatus::Approved,
            Self::Reject => ScholarshipAwardStatus::Rejected,
            Self::Disburse => ScholarshipAwardStatus::Disbursed,
            Self::Revoke => ScholarshipAwardStatus::Revoked,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScholarshipRuleViolation {
    Forbidden,
    PoolClosed,
    AmountMustBePositive,
    CurrencyMismatch,
    InsufficientPoolBalance,
    ChildProfileAccessDenied,
    GeographyRestricted,
    EducationLevelRestricted,
    SchoolRestricted,
    CategoryRestricted,
    InvalidAwardTransition,
    MissingDisbursementTarget,
}

impl fmt::Display for ScholarshipRuleViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forbidden => write!(f, "actor is not allowed to perform this action"),
            Self::PoolClosed => write!(f, "scholarship pool is not open"),
            Self::AmountMustBePositive => write!(f, "amount must be positive"),
            Self::CurrencyMismatch => write!(f, "currency must match scholarship pool currency"),
            Self::InsufficientPoolBalance => {
                write!(f, "award amount exceeds available scholarship pool balance")
            }
            Self::ChildProfileAccessDenied => {
                write!(f, "actor cannot apply with the specified child profile")
            }
            Self::GeographyRestricted => {
                write!(f, "application does not satisfy geography restriction")
            }
            Self::EducationLevelRestricted => {
                write!(f, "application does not satisfy education level restriction")
            }
            Self::SchoolRestricted => write!(f, "application does not satisfy school restriction"),
            Self::CategoryRestricted => {
                write!(f, "application does not satisfy category restriction")
            }
            Self::InvalidAwardTransition => write!(f, "invalid scholarship award transition"),
            Self::MissingDisbursementTarget => {
                write!(f, "disbursed award must link to a payout request or vault earmark")
            }
        }
    }
}

impl std::error::Error for ScholarshipRuleViolation {}

pub fn validate_pool_is_open(pool: &ScholarshipPool) -> Result<(), ScholarshipRuleViolation> {
    if pool.status == ScholarshipPoolStatus::Open {
        Ok(())
    } else {
        Err(ScholarshipRuleViolation::PoolClosed)
    }
}

pub fn validate_positive_amount(amount_minor: i64) -> Result<(), ScholarshipRuleViolation> {
    if amount_minor > 0 { Ok(()) } else { Err(ScholarshipRuleViolation::AmountMustBePositive) }
}

pub fn validate_currency(
    pool_currency: &str,
    value_currency: &str,
) -> Result<(), ScholarshipRuleViolation> {
    if pool_currency == value_currency {
        Ok(())
    } else {
        Err(ScholarshipRuleViolation::CurrencyMismatch)
    }
}

pub fn ensure_pool_balance(
    available_minor: i64,
    amount_minor: i64,
) -> Result<(), ScholarshipRuleViolation> {
    if amount_minor <= available_minor {
        Ok(())
    } else {
        Err(ScholarshipRuleViolation::InsufficientPoolBalance)
    }
}

pub fn can_fund_pool(role: UserRole) -> Result<(), ScholarshipRuleViolation> {
    if matches!(role, UserRole::Donor | UserRole::PlatformAdmin) {
        Ok(())
    } else {
        Err(ScholarshipRuleViolation::Forbidden)
    }
}

pub fn can_apply(role: UserRole) -> Result<(), ScholarshipRuleViolation> {
    if matches!(role, UserRole::Parent | UserRole::Student) {
        Ok(())
    } else {
        Err(ScholarshipRuleViolation::Forbidden)
    }
}

pub fn can_review(role: UserRole) -> Result<(), ScholarshipRuleViolation> {
    if role == UserRole::PlatformAdmin { Ok(()) } else { Err(ScholarshipRuleViolation::Forbidden) }
}

pub fn validate_child_profile_access(
    role: UserRole,
    actor_user_id: Uuid,
    child_owner_user_id: Uuid,
) -> Result<(), ScholarshipRuleViolation> {
    if role == UserRole::Student || actor_user_id == child_owner_user_id {
        Ok(())
    } else {
        Err(ScholarshipRuleViolation::ChildProfileAccessDenied)
    }
}

pub fn validate_pool_restrictions(
    pool: &ScholarshipPool,
    geography: Option<&str>,
    education_level: Option<&str>,
    school_id: Option<Uuid>,
    category: Option<&str>,
) -> Result<(), ScholarshipRuleViolation> {
    if let Some(expected) = pool.geography_restriction.as_deref() {
        if geography.map(str::trim) != Some(expected) {
            return Err(ScholarshipRuleViolation::GeographyRestricted);
        }
    }

    if let Some(expected) = pool.education_level_restriction.as_deref() {
        if education_level.map(str::trim) != Some(expected) {
            return Err(ScholarshipRuleViolation::EducationLevelRestricted);
        }
    }

    if let Some(expected) = pool.school_id_restriction {
        if school_id != Some(expected) {
            return Err(ScholarshipRuleViolation::SchoolRestricted);
        }
    }

    if let Some(expected) = pool.category_restriction.as_deref() {
        if category.map(str::trim) != Some(expected) {
            return Err(ScholarshipRuleViolation::CategoryRestricted);
        }
    }

    Ok(())
}

pub fn validate_award_transition(
    current: ScholarshipAwardStatus,
    transition: AwardTransition,
    linked_payout_request_id: Option<Uuid>,
    linked_vault_id: Option<Uuid>,
) -> Result<ScholarshipAwardStatus, ScholarshipRuleViolation> {
    let target = transition.target_status();

    match (current, target) {
        (ScholarshipAwardStatus::Approved, ScholarshipAwardStatus::Disbursed) => {
            if linked_payout_request_id.is_none() && linked_vault_id.is_none() {
                return Err(ScholarshipRuleViolation::MissingDisbursementTarget);
            }
            Ok(target)
        }
        (ScholarshipAwardStatus::Approved, ScholarshipAwardStatus::Revoked) => Ok(target),
        (from, to) if from == to => Ok(to),
        _ => Err(ScholarshipRuleViolation::InvalidAwardTransition),
    }
}

pub fn validate_application_reviewable(
    status: ScholarshipApplicationStatus,
) -> Result<(), ScholarshipRuleViolation> {
    if matches!(
        status,
        ScholarshipApplicationStatus::Submitted | ScholarshipApplicationStatus::UnderReview
    ) {
        Ok(())
    } else {
        Err(ScholarshipRuleViolation::InvalidAwardTransition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_platform_admin_can_review() {
        assert!(can_review(UserRole::Parent).is_err());
        assert!(can_review(UserRole::PlatformAdmin).is_ok());
    }

    #[test]
    fn disbursement_requires_linked_target() {
        let result = validate_award_transition(
            ScholarshipAwardStatus::Approved,
            AwardTransition::Disburse,
            None,
            None,
        );
        assert!(matches!(result, Err(ScholarshipRuleViolation::MissingDisbursementTarget)));
    }

    #[test]
    fn scholarship_pool_restrictions_reject_wrong_category() {
        let pool = ScholarshipPool {
            id: Uuid::new_v4(),
            owner_user_id: Uuid::new_v4(),
            name: "STEM".to_owned(),
            description: None,
            status: ScholarshipPoolStatus::Open,
            available_funds: shared::money::Money::new(10_000, shared::currency::Currency::Fiat("USD".to_owned())).unwrap(),
            geography_restriction: None,
            education_level_restriction: None,
            school_id_restriction: None,
            category_restriction: Some("stem".to_owned()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let result = validate_pool_restrictions(&pool, None, None, None, Some("arts"));
        assert!(matches!(result, Err(ScholarshipRuleViolation::CategoryRestricted)));
    }
}
