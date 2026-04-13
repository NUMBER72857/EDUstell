use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::money::Money;
use uuid::Uuid;

use crate::{auth::UserRole, persistence::ContributionStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VaultLedgerEntryType {
    ContributionConfirmed,
    ContributionReversed,
}

impl VaultLedgerEntryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ContributionConfirmed => "contribution_confirmed",
            Self::ContributionReversed => "contribution_reversed",
        }
    }
}

impl fmt::Display for VaultLedgerEntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for VaultLedgerEntryType {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "contribution_confirmed" => Ok(Self::ContributionConfirmed),
            "contribution_reversed" => Ok(Self::ContributionReversed),
            _ => Err("invalid vault ledger entry type"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultLedgerEntry {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub contribution_id: Option<Uuid>,
    pub actor_user_id: Option<Uuid>,
    pub entry_type: VaultLedgerEntryType,
    pub amount: Money,
    pub balance_after_minor: i64,
    pub external_reference: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContributionTransition {
    Confirm,
    Fail,
    Reverse,
}

impl ContributionTransition {
    pub fn target_status(&self) -> ContributionStatus {
        match self {
            Self::Confirm => ContributionStatus::Confirmed,
            Self::Fail => ContributionStatus::Failed,
            Self::Reverse => ContributionStatus::Reversed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContributionRuleViolation {
    AmountMustBePositive,
    CurrencyMismatch,
    UnauthorizedContributor,
    UnauthorizedSettlementActor,
    InvalidStatusTransition { from: ContributionStatus, to: ContributionStatus },
}

impl fmt::Display for ContributionRuleViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AmountMustBePositive => write!(f, "contribution amount must be positive"),
            Self::CurrencyMismatch => write!(f, "contribution currency must match vault currency"),
            Self::UnauthorizedContributor => {
                write!(f, "user is not allowed to contribute to this vault")
            }
            Self::UnauthorizedSettlementActor => {
                write!(f, "user is not allowed to settle contributions")
            }
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "invalid contribution transition from {} to {}", from, to)
            }
        }
    }
}

impl std::error::Error for ContributionRuleViolation {}

pub fn validate_contribution_amount(amount_minor: i64) -> Result<(), ContributionRuleViolation> {
    if amount_minor <= 0 {
        return Err(ContributionRuleViolation::AmountMustBePositive);
    }

    Ok(())
}

pub fn validate_contribution_currency(
    contribution_currency: &str,
    vault_currency: &str,
) -> Result<(), ContributionRuleViolation> {
    if contribution_currency != vault_currency {
        return Err(ContributionRuleViolation::CurrencyMismatch);
    }

    Ok(())
}

pub fn can_view_or_fund_vault(
    actor_role: UserRole,
    actor_user_id: Uuid,
    vault_owner_user_id: Uuid,
    is_explicit_contributor: bool,
) -> Result<(), ContributionRuleViolation> {
    if actor_user_id == vault_owner_user_id && actor_role == UserRole::Parent {
        return Ok(());
    }

    if is_explicit_contributor && matches!(actor_role, UserRole::Contributor | UserRole::Parent) {
        return Ok(());
    }

    Err(ContributionRuleViolation::UnauthorizedContributor)
}

pub fn can_settle_contribution(actor_role: UserRole) -> Result<(), ContributionRuleViolation> {
    if actor_role == UserRole::PlatformAdmin {
        return Ok(());
    }

    Err(ContributionRuleViolation::UnauthorizedSettlementActor)
}

pub fn validate_transition(
    current: ContributionStatus,
    transition: ContributionTransition,
) -> Result<ContributionStatus, ContributionRuleViolation> {
    let target = transition.target_status();

    match (current, target) {
        (ContributionStatus::Pending, ContributionStatus::Confirmed)
        | (ContributionStatus::Pending, ContributionStatus::Failed)
        | (ContributionStatus::Confirmed, ContributionStatus::Reversed) => Ok(target),
        (from, to) if from == to => Ok(to),
        (from, to) => Err(ContributionRuleViolation::InvalidStatusTransition { from, to }),
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[test]
    fn parent_owner_can_fund_vault() {
        let result = can_view_or_fund_vault(UserRole::Parent, Uuid::nil(), Uuid::nil(), false);

        assert!(result.is_ok());
    }

    #[test]
    fn only_platform_admin_can_settle() {
        assert!(can_settle_contribution(UserRole::Contributor).is_err());
        assert!(can_settle_contribution(UserRole::PlatformAdmin).is_ok());
    }

    #[test]
    fn explicit_contributor_can_fund_vault() {
        let result = can_view_or_fund_vault(
            UserRole::Contributor,
            Uuid::new_v4(),
            Uuid::new_v4(),
            true,
        );

        assert!(result.is_ok());
    }

    #[test]
    fn confirmed_contribution_cannot_fail() {
        let result =
            validate_transition(ContributionStatus::Confirmed, ContributionTransition::Fail);

        assert!(matches!(
            result,
            Err(ContributionRuleViolation::InvalidStatusTransition { .. })
        ));
    }
}
