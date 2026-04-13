use std::fmt;

use crate::{
    auth::UserRole,
    persistence::{MilestoneStatus, PayoutStatus, SchoolVerificationStatus},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayoutTransition {
    MoveToReview,
    Approve,
    Reject,
    MarkProcessing,
    Complete,
    Fail,
}

impl PayoutTransition {
    pub fn target_status(self) -> PayoutStatus {
        match self {
            Self::MoveToReview => PayoutStatus::UnderReview,
            Self::Approve => PayoutStatus::Approved,
            Self::Reject => PayoutStatus::Rejected,
            Self::MarkProcessing => PayoutStatus::Processing,
            Self::Complete => PayoutStatus::Completed,
            Self::Fail => PayoutStatus::Failed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayoutRuleViolation {
    AmountMustBePositive,
    UnauthorizedRequester,
    UnauthorizedReviewer,
    SchoolNotVerified,
    MilestoneDoesNotBelongToVault,
    MilestoneNotPayable,
    CurrencyMismatch,
    InsufficientAvailableFunds,
    InvalidStatusTransition { from: PayoutStatus, to: PayoutStatus },
}

impl fmt::Display for PayoutRuleViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmountMustBePositive => write!(f, "payout amount must be positive"),
            Self::UnauthorizedRequester => write!(f, "user cannot request payout"),
            Self::UnauthorizedReviewer => write!(f, "user cannot review payout"),
            Self::SchoolNotVerified => write!(f, "school is not verified"),
            Self::MilestoneDoesNotBelongToVault => {
                write!(f, "milestone does not belong to the specified vault")
            }
            Self::MilestoneNotPayable => write!(f, "milestone is not payable in its current state"),
            Self::CurrencyMismatch => write!(f, "payout currency must match vault currency"),
            Self::InsufficientAvailableFunds => write!(f, "vault has insufficient available funds"),
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "invalid payout transition from {} to {}", from, to)
            }
        }
    }
}

impl std::error::Error for PayoutRuleViolation {}

pub fn validate_requester_role(role: UserRole) -> Result<(), PayoutRuleViolation> {
    if role == UserRole::Parent {
        return Ok(());
    }

    Err(PayoutRuleViolation::UnauthorizedRequester)
}

pub fn validate_reviewer_role(role: UserRole) -> Result<(), PayoutRuleViolation> {
    if role == UserRole::PlatformAdmin {
        return Ok(());
    }

    Err(PayoutRuleViolation::UnauthorizedReviewer)
}

pub fn validate_school_verified(
    status: SchoolVerificationStatus,
) -> Result<(), PayoutRuleViolation> {
    if status == SchoolVerificationStatus::Verified {
        return Ok(());
    }

    Err(PayoutRuleViolation::SchoolNotVerified)
}

pub fn validate_payout_amount(amount_minor: i64) -> Result<(), PayoutRuleViolation> {
    if amount_minor <= 0 {
        return Err(PayoutRuleViolation::AmountMustBePositive);
    }

    Ok(())
}

pub fn validate_currency_match(
    payout_currency: &str,
    vault_currency: &str,
) -> Result<(), PayoutRuleViolation> {
    if payout_currency == vault_currency {
        return Ok(());
    }

    Err(PayoutRuleViolation::CurrencyMismatch)
}

pub fn validate_milestone_association(
    milestone_vault_id: uuid::Uuid,
    requested_vault_id: uuid::Uuid,
) -> Result<(), PayoutRuleViolation> {
    if milestone_vault_id == requested_vault_id {
        return Ok(());
    }

    Err(PayoutRuleViolation::MilestoneDoesNotBelongToVault)
}

pub fn validate_milestone_payable(status: MilestoneStatus) -> Result<(), PayoutRuleViolation> {
    if matches!(
        status,
        MilestoneStatus::Planned | MilestoneStatus::Funded | MilestoneStatus::PartiallyPaid
    ) {
        return Ok(());
    }

    Err(PayoutRuleViolation::MilestoneNotPayable)
}

pub fn ensure_available_funds(
    total_contributed_minor: i64,
    total_locked_minor: i64,
    total_disbursed_minor: i64,
    payout_amount_minor: i64,
) -> Result<(), PayoutRuleViolation> {
    let available = total_contributed_minor - total_locked_minor - total_disbursed_minor;
    if available >= payout_amount_minor {
        return Ok(());
    }

    Err(PayoutRuleViolation::InsufficientAvailableFunds)
}

pub fn validate_transition(
    current: PayoutStatus,
    transition: PayoutTransition,
) -> Result<PayoutStatus, PayoutRuleViolation> {
    let target = transition.target_status();

    match (current, target) {
        (PayoutStatus::Pending, PayoutStatus::UnderReview)
        | (PayoutStatus::Pending, PayoutStatus::Rejected)
        | (PayoutStatus::UnderReview, PayoutStatus::Approved)
        | (PayoutStatus::UnderReview, PayoutStatus::Rejected)
        | (PayoutStatus::Approved, PayoutStatus::Processing)
        | (PayoutStatus::Processing, PayoutStatus::Completed)
        | (PayoutStatus::Processing, PayoutStatus::Failed) => Ok(target),
        (from, to) if from == to => Ok(to),
        (from, to) => Err(PayoutRuleViolation::InvalidStatusTransition { from, to }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payout_must_not_exceed_available_balance() {
        let result = ensure_available_funds(10_000, 2_000, 3_000, 6_000);
        assert!(matches!(result, Err(PayoutRuleViolation::InsufficientAvailableFunds)));
    }

    #[test]
    fn payout_requires_review_state_before_approval() {
        let result = validate_transition(PayoutStatus::Pending, PayoutTransition::Approve);
        assert!(matches!(result, Err(PayoutRuleViolation::InvalidStatusTransition { .. })));
    }

    #[test]
    fn payout_requires_verified_school() {
        let result = validate_school_verified(SchoolVerificationStatus::Pending);
        assert!(matches!(result, Err(PayoutRuleViolation::SchoolNotVerified)));
    }

    #[test]
    fn only_parent_can_request_payout() {
        assert!(validate_requester_role(crate::auth::UserRole::Contributor).is_err());
        assert!(validate_requester_role(crate::auth::UserRole::Parent).is_ok());
    }
}
