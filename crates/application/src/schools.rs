use chrono::Utc;
use domain::{
    auth::{AuthenticatedUser, UserRole},
    persistence::{School, SchoolPayoutMethod, SchoolVerificationStatus},
};
use serde_json::json;
use uuid::Uuid;

use crate::audit::AuditEvent;
use crate::repos::{PersistenceError, SchoolRepository, SchoolWorkflowRepository};

#[derive(Debug, thiserror::Error)]
pub enum SchoolError {
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

impl From<PersistenceError> for SchoolError {
    fn from(value: PersistenceError) -> Self {
        match value {
            PersistenceError::NotFound => Self::NotFound,
            PersistenceError::Conflict(message) => Self::Conflict(message),
            PersistenceError::Validation(message) => Self::Validation(message),
            PersistenceError::Repository(message) => Self::Repository(message),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateSchoolInput {
    pub legal_name: String,
    pub display_name: String,
    pub country: String,
    pub payout_method: SchoolPayoutMethod,
    pub payout_reference: String,
}

#[derive(Debug, Clone)]
pub struct VerifySchoolInput {
    pub school_id: Uuid,
    pub verification_status: SchoolVerificationStatus,
}

pub async fn create_school<W>(
    workflow: &W,
    actor: &AuthenticatedUser,
    input: CreateSchoolInput,
) -> Result<School, SchoolError>
where
    W: SchoolWorkflowRepository,
{
    require_platform_admin(actor.role)?;
    validate_school_fields(&input)?;

    let now = Utc::now();
    let school = School {
        id: Uuid::new_v4(),
        legal_name: input.legal_name,
        display_name: input.display_name,
        country: input.country,
        payout_method: input.payout_method,
        payout_reference: input.payout_reference,
        verification_status: SchoolVerificationStatus::Pending,
        verified_by: None,
        verified_at: None,
        created_at: now,
        updated_at: now,
    };
    let mut audit = AuditEvent::school_verified(actor.user_id, &school).into_log();
    audit.action = "school.created".to_owned();
    audit.metadata = json!({
        "display_name": school.display_name,
        "country": school.country,
        "payout_method": school.payout_method.as_str(),
        "verification_status": school.verification_status.as_str(),
    });

    workflow.create_with_audit(school, audit).await.map_err(Into::into)
}

pub async fn verify_school<W>(
    workflow: &W,
    actor: &AuthenticatedUser,
    input: VerifySchoolInput,
) -> Result<School, SchoolError>
where
    W: SchoolWorkflowRepository,
{
    require_platform_admin(actor.role)?;

    let mut audit = AuditEvent::new(
        Some(actor.user_id),
        crate::audit::AuditEntityType::School,
        Some(input.school_id),
        crate::audit::AuditAction::SchoolVerified,
        json!({
            "verification_status": input.verification_status.as_str(),
        }),
    )
    .into_log();
    audit.action = "school.verification_status_changed".to_owned();

    workflow
        .verify_with_audit(
            input.school_id,
            input.verification_status.as_str(),
            actor.user_id,
            audit,
        )
        .await
        .map_err(Into::into)
}

pub async fn search_verified_schools<R>(
    schools: &R,
    actor: &AuthenticatedUser,
    query: Option<&str>,
) -> Result<Vec<School>, SchoolError>
where
    R: SchoolRepository,
{
    if !matches!(actor.role, UserRole::Parent | UserRole::PlatformAdmin) {
        return Err(SchoolError::Forbidden);
    }

    schools.search_verified(query).await.map_err(Into::into)
}

fn require_platform_admin(role: UserRole) -> Result<(), SchoolError> {
    if role == UserRole::PlatformAdmin {
        return Ok(());
    }

    Err(SchoolError::Forbidden)
}

fn validate_school_fields(input: &CreateSchoolInput) -> Result<(), SchoolError> {
    if input.legal_name.trim().is_empty() {
        return Err(SchoolError::Validation("legal_name is required".to_owned()));
    }
    if input.display_name.trim().is_empty() {
        return Err(SchoolError::Validation("display_name is required".to_owned()));
    }
    if input.country.trim().is_empty() {
        return Err(SchoolError::Validation("country is required".to_owned()));
    }
    if input.payout_reference.trim().is_empty() {
        return Err(SchoolError::Validation("payout_reference is required".to_owned()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use chrono::Utc;
    use domain::auth::{AuthenticatedUser, UserRole};
    use tokio::sync::Mutex;

    use super::*;
    use crate::repos::{PersistenceError, SchoolWorkflowRepository};

    #[derive(Default)]
    struct State {
        schools: HashMap<Uuid, School>,
        audits: Vec<domain::persistence::AuditLog>,
    }

    #[derive(Clone, Default)]
    struct FakeWorkflow {
        state: Arc<Mutex<State>>,
    }

    #[async_trait]
    impl SchoolWorkflowRepository for FakeWorkflow {
        async fn create_with_audit(
            &self,
            school: School,
            audit_log: domain::persistence::AuditLog,
        ) -> Result<School, PersistenceError> {
            let mut state = self.state.lock().await;
            state.schools.insert(school.id, school.clone());
            state.audits.push(audit_log);
            Ok(school)
        }

        async fn verify_with_audit(
            &self,
            school_id: Uuid,
            verification_status: &str,
            verified_by: Uuid,
            audit_log: domain::persistence::AuditLog,
        ) -> Result<School, PersistenceError> {
            let mut state = self.state.lock().await;
            let school = state.schools.get_mut(&school_id).ok_or(PersistenceError::NotFound)?;
            school.verification_status = verification_status
                .parse()
                .map_err(|_| PersistenceError::Validation("invalid verification status".to_owned()))?;
            school.verified_by = Some(verified_by);
            school.verified_at = Some(Utc::now());
            let updated = school.clone();
            state.audits.push(audit_log);
            Ok(updated)
        }
    }

    fn admin() -> AuthenticatedUser {
        AuthenticatedUser {
            user_id: Uuid::new_v4(),
            role: UserRole::PlatformAdmin,
            session_id: Uuid::new_v4(),
        }
    }

    #[tokio::test]
    async fn create_school_records_creation_audit() {
        let workflow = FakeWorkflow::default();
        let school = create_school(
            &workflow,
            &admin(),
            CreateSchoolInput {
                legal_name: "Springfield High".to_owned(),
                display_name: "Springfield High".to_owned(),
                country: "NG".to_owned(),
                payout_method: SchoolPayoutMethod::Manual,
                payout_reference: "acct-001".to_owned(),
            },
        )
        .await
        .unwrap();

        let state = workflow.state.lock().await;
        assert_eq!(school.verification_status, SchoolVerificationStatus::Pending);
        assert_eq!(state.audits.len(), 1);
        assert_eq!(state.audits[0].action, "school.created");
        assert_eq!(state.audits[0].metadata["payout_method"], "manual");
    }

    #[tokio::test]
    async fn verify_school_records_status_change_audit() {
        let workflow = FakeWorkflow::default();
        let actor = admin();
        let school_id = Uuid::new_v4();

        workflow.state.lock().await.schools.insert(
            school_id,
            School {
                id: school_id,
                legal_name: "Springfield High".to_owned(),
                display_name: "Springfield High".to_owned(),
                country: "NG".to_owned(),
                payout_method: SchoolPayoutMethod::Manual,
                payout_reference: "acct-001".to_owned(),
                verification_status: SchoolVerificationStatus::Pending,
                verified_by: None,
                verified_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        );

        let school = verify_school(
            &workflow,
            &actor,
            VerifySchoolInput {
                school_id,
                verification_status: SchoolVerificationStatus::Verified,
            },
        )
        .await
        .unwrap();

        let state = workflow.state.lock().await;
        assert_eq!(school.verification_status, SchoolVerificationStatus::Verified);
        assert_eq!(state.audits.len(), 1);
        assert_eq!(state.audits[0].action, "school.verification_status_changed");
        assert_eq!(state.audits[0].metadata["verification_status"], "verified");
    }
}
