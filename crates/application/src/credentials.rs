use chrono::{NaiveDate, Utc};
use domain::{
    auth::{AuthenticatedUser, UserRole},
    persistence::{
        AchievementCredential, AchievementCredentialStatus, AchievementCredentialType, ChildProfile,
        SchoolVerificationStatus,
    },
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    audit::AuditEvent,
    repos::{
        AchievementCredentialRepository, ChildProfileRepository, PersistenceError, SchoolRepository,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
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

impl From<PersistenceError> for CredentialError {
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
pub struct IssueCredentialInput {
    pub child_profile_id: Uuid,
    pub recipient_user_id: Option<Uuid>,
    pub school_id: Option<Uuid>,
    pub achievement_type: AchievementCredentialType,
    pub title: String,
    pub description: Option<String>,
    pub achievement_date: NaiveDate,
    pub issuance_notes: Option<String>,
    pub evidence_uri: Option<String>,
    pub attestation_anchor: Option<String>,
    pub attestation_anchor_network: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Default)]
pub struct CredentialListFilter {
    pub child_profile_id: Option<Uuid>,
    pub issued_by_me: bool,
}

pub async fn issue_credential<C, S, R>(
    child_profiles: &C,
    schools: &S,
    credentials: &R,
    actor: &AuthenticatedUser,
    input: IssueCredentialInput,
) -> Result<AchievementCredential, CredentialError>
where
    C: ChildProfileRepository,
    S: SchoolRepository,
    R: AchievementCredentialRepository,
{
    let child = child_profiles
        .find_by_id(input.child_profile_id)
        .await?
        .ok_or(CredentialError::NotFound)?;
    validate_issuer(actor, schools, input.school_id).await?;
    validate_input(&input, &child)?;

    let now = Utc::now();
    let credential_ref = Uuid::new_v4();
    let attestation_hash = build_attestation_hash(&credential_ref, &input, actor.user_id);
    let credential = AchievementCredential {
        id: Uuid::new_v4(),
        credential_ref,
        child_profile_id: input.child_profile_id,
        recipient_user_id: input.recipient_user_id,
        school_id: input.school_id,
        achievement_type: input.achievement_type,
        status: AchievementCredentialStatus::Issued,
        title: input.title.trim().to_owned(),
        description: sanitize_optional(input.description),
        achievement_date: input.achievement_date,
        issued_by_user_id: actor.user_id,
        issued_by_role: actor.role.as_str().to_owned(),
        issuance_notes: sanitize_optional(input.issuance_notes),
        evidence_uri: sanitize_optional(input.evidence_uri),
        attestation_hash,
        attestation_method: "sha256".to_owned(),
        attestation_anchor: sanitize_optional(input.attestation_anchor),
        attestation_anchor_network: sanitize_optional(input.attestation_anchor_network),
        metadata: input.metadata,
        created_at: now,
        updated_at: now,
    };
    let audit = AuditEvent::achievement_credential_issued(actor.user_id, &credential).into_log();

    credentials.create_with_audit(credential, audit).await.map_err(Into::into)
}

pub async fn list_credentials<C, R>(
    child_profiles: &C,
    credentials: &R,
    actor: &AuthenticatedUser,
    filter: CredentialListFilter,
) -> Result<Vec<AchievementCredential>, CredentialError>
where
    C: ChildProfileRepository,
    R: AchievementCredentialRepository,
{
    let mut items = match actor.role {
        UserRole::Parent => {
            if let Some(child_profile_id) = filter.child_profile_id {
                let child = child_profiles
                    .find_by_id(child_profile_id)
                    .await?
                    .ok_or(CredentialError::NotFound)?;
                if child.owner_user_id != actor.user_id {
                    return Err(CredentialError::Forbidden);
                }
                credentials.list_by_child_profile(child_profile_id).await?
            } else {
                let children = child_profiles.list_by_owner(actor.user_id).await?;
                let mut merged = Vec::new();
                for child in children {
                    merged.extend(credentials.list_by_child_profile(child.id).await?);
                }
                merged
            }
        }
        UserRole::Student => {
            if filter.child_profile_id.is_some() || filter.issued_by_me {
                return Err(CredentialError::Forbidden);
            }
            credentials.list_by_recipient(actor.user_id).await?
        }
        UserRole::SchoolAdmin => {
            if filter.child_profile_id.is_some() {
                return Err(CredentialError::Forbidden);
            }
            credentials.list_by_issuer(actor.user_id).await?
        }
        UserRole::PlatformAdmin => {
            if filter.issued_by_me {
                credentials.list_by_issuer(actor.user_id).await?
            } else if let Some(child_profile_id) = filter.child_profile_id {
                credentials.list_by_child_profile(child_profile_id).await?
            } else {
                credentials.list_recent(200).await?
            }
        }
        UserRole::Contributor | UserRole::Donor => return Err(CredentialError::Forbidden),
    };

    items.sort_by(|left, right| {
        right
            .achievement_date
            .cmp(&left.achievement_date)
            .then_with(|| right.created_at.cmp(&left.created_at))
    });
    items.dedup_by_key(|item| item.id);

    Ok(items)
}

pub async fn get_credential<C, R>(
    child_profiles: &C,
    credentials: &R,
    actor: &AuthenticatedUser,
    credential_id: Uuid,
) -> Result<AchievementCredential, CredentialError>
where
    C: ChildProfileRepository,
    R: AchievementCredentialRepository,
{
    let credential = credentials
        .find_by_id(credential_id)
        .await?
        .ok_or(CredentialError::NotFound)?;

    match actor.role {
        UserRole::PlatformAdmin => Ok(credential),
        UserRole::Student if credential.recipient_user_id == Some(actor.user_id) => Ok(credential),
        UserRole::SchoolAdmin if credential.issued_by_user_id == actor.user_id => Ok(credential),
        UserRole::Parent => {
            let child = child_profiles
                .find_by_id(credential.child_profile_id)
                .await?
                .ok_or(CredentialError::NotFound)?;
            if child.owner_user_id == actor.user_id {
                Ok(credential)
            } else {
                Err(CredentialError::Forbidden)
            }
        }
        _ => Err(CredentialError::Forbidden),
    }
}

async fn validate_issuer<S>(
    actor: &AuthenticatedUser,
    schools: &S,
    school_id: Option<Uuid>,
) -> Result<(), CredentialError>
where
    S: SchoolRepository,
{
    match actor.role {
        UserRole::PlatformAdmin => Ok(()),
        UserRole::SchoolAdmin => {
            let school_id = school_id.ok_or_else(|| {
                CredentialError::Validation(
                    "school_id is required when issuing as school_admin".to_owned(),
                )
            })?;
            let school = schools.find_by_id(school_id).await?.ok_or(CredentialError::NotFound)?;
            if school.verification_status != SchoolVerificationStatus::Verified {
                return Err(CredentialError::Conflict(
                    "school must be verified before issuing credentials".to_owned(),
                ));
            }
            Ok(())
        }
        _ => Err(CredentialError::Forbidden),
    }
}

fn validate_input(input: &IssueCredentialInput, child: &ChildProfile) -> Result<(), CredentialError> {
    if input.title.trim().is_empty() {
        return Err(CredentialError::Validation("title is required".to_owned()));
    }
    if let Some(recipient_user_id) = input.recipient_user_id {
        if child.owner_user_id == recipient_user_id {
            return Err(CredentialError::Validation(
                "recipient_user_id must identify the student, not the guardian".to_owned(),
            ));
        }
    }
    if input.attestation_anchor.is_some() && input.attestation_anchor_network.is_none() {
        return Err(CredentialError::Validation(
            "attestation_anchor_network is required when attestation_anchor is set".to_owned(),
        ));
    }
    Ok(())
}

fn build_attestation_hash(
    credential_ref: &Uuid,
    input: &IssueCredentialInput,
    issued_by_user_id: Uuid,
) -> String {
    let canonical = serde_json::json!({
        "credential_ref": credential_ref,
        "child_profile_id": input.child_profile_id,
        "recipient_user_id": input.recipient_user_id,
        "school_id": input.school_id,
        "achievement_type": input.achievement_type.as_str(),
        "title": input.title.trim(),
        "description": sanitize_optional(input.description.clone()),
        "achievement_date": input.achievement_date,
        "issued_by_user_id": issued_by_user_id,
        "issuance_notes": sanitize_optional(input.issuance_notes.clone()),
        "evidence_uri": sanitize_optional(input.evidence_uri.clone()),
        "metadata": input.metadata,
    })
    .to_string();

    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("{:x}", hasher.finalize())
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
    use chrono::NaiveDate;
    use domain::{
        auth::{AuthenticatedUser, UserRole},
        persistence::{
            AchievementCredential, ChildProfile, School, SchoolPayoutMethod,
            SchoolVerificationStatus,
        },
    };
    use tokio::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct State {
        children: HashMap<Uuid, ChildProfile>,
        schools: HashMap<Uuid, School>,
        credentials: HashMap<Uuid, AchievementCredential>,
    }

    #[derive(Clone, Default)]
    struct FakeRepos {
        state: Arc<Mutex<State>>,
    }

    #[async_trait]
    impl ChildProfileRepository for FakeRepos {
        async fn create(&self, profile: ChildProfile) -> Result<ChildProfile, PersistenceError> {
            self.state.lock().await.children.insert(profile.id, profile.clone());
            Ok(profile)
        }

        async fn find_by_id(&self, id: Uuid) -> Result<Option<ChildProfile>, PersistenceError> {
            Ok(self.state.lock().await.children.get(&id).cloned())
        }

        async fn list_by_owner(
            &self,
            owner_user_id: Uuid,
        ) -> Result<Vec<ChildProfile>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .children
                .values()
                .filter(|item| item.owner_user_id == owner_user_id)
                .cloned()
                .collect())
        }
    }

    #[async_trait]
    impl SchoolRepository for FakeRepos {
        async fn create(&self, school: School) -> Result<School, PersistenceError> {
            self.state.lock().await.schools.insert(school.id, school.clone());
            Ok(school)
        }

        async fn find_by_id(&self, id: Uuid) -> Result<Option<School>, PersistenceError> {
            Ok(self.state.lock().await.schools.get(&id).cloned())
        }

        async fn search_verified(&self, _query: Option<&str>) -> Result<Vec<School>, PersistenceError> {
            Ok(vec![])
        }

        async fn list_verified(&self) -> Result<Vec<School>, PersistenceError> {
            Ok(vec![])
        }

        async fn update_verification(
            &self,
            _id: Uuid,
            _verification_status: &str,
            _verified_by: Option<Uuid>,
            _verified_at: Option<chrono::DateTime<chrono::Utc>>,
        ) -> Result<(), PersistenceError> {
            Ok(())
        }
    }

    #[async_trait]
    impl AchievementCredentialRepository for FakeRepos {
        async fn create_with_audit(
            &self,
            credential: AchievementCredential,
            _audit_log: domain::persistence::AuditLog,
        ) -> Result<AchievementCredential, PersistenceError> {
            self.state
                .lock()
                .await
                .credentials
                .insert(credential.id, credential.clone());
            Ok(credential)
        }

        async fn find_by_id(
            &self,
            id: Uuid,
        ) -> Result<Option<AchievementCredential>, PersistenceError> {
            Ok(self.state.lock().await.credentials.get(&id).cloned())
        }

        async fn list_by_child_profile(
            &self,
            child_profile_id: Uuid,
        ) -> Result<Vec<AchievementCredential>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .credentials
                .values()
                .filter(|item| item.child_profile_id == child_profile_id)
                .cloned()
                .collect())
        }

        async fn list_by_recipient(
            &self,
            recipient_user_id: Uuid,
        ) -> Result<Vec<AchievementCredential>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .credentials
                .values()
                .filter(|item| item.recipient_user_id == Some(recipient_user_id))
                .cloned()
                .collect())
        }

        async fn list_by_issuer(
            &self,
            issued_by_user_id: Uuid,
        ) -> Result<Vec<AchievementCredential>, PersistenceError> {
            Ok(self
                .state
                .lock()
                .await
                .credentials
                .values()
                .filter(|item| item.issued_by_user_id == issued_by_user_id)
                .cloned()
                .collect())
        }

        async fn list_recent(
            &self,
            _limit: i64,
        ) -> Result<Vec<AchievementCredential>, PersistenceError> {
            Ok(self.state.lock().await.credentials.values().cloned().collect())
        }
    }

    fn make_child(owner_user_id: Uuid) -> ChildProfile {
        ChildProfile {
            id: Uuid::new_v4(),
            owner_user_id,
            full_name: "Student".to_owned(),
            date_of_birth: None,
            education_level: Some("secondary".to_owned()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_school() -> School {
        School {
            id: Uuid::new_v4(),
            legal_name: "Springfield High".to_owned(),
            display_name: "Springfield High".to_owned(),
            country: "NG".to_owned(),
            payout_method: SchoolPayoutMethod::Manual,
            payout_reference: "acct-1".to_owned(),
            verification_status: SchoolVerificationStatus::Verified,
            verified_by: Some(Uuid::new_v4()),
            verified_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn school_admin_can_issue_credential_for_verified_school() {
        let repos = FakeRepos::default();
        let parent_id = Uuid::new_v4();
        let student_id = Uuid::new_v4();
        let child = make_child(parent_id);
        let school = make_school();
        crate::repos::ChildProfileRepository::create(&repos, child.clone()).await.unwrap();
        crate::repos::SchoolRepository::create(&repos, school.clone()).await.unwrap();

        let credential = issue_credential(
            &repos,
            &repos,
            &repos,
            &AuthenticatedUser {
                user_id: Uuid::new_v4(),
                role: UserRole::SchoolAdmin,
                session_id: Uuid::new_v4(),
            },
            IssueCredentialInput {
                child_profile_id: child.id,
                recipient_user_id: Some(student_id),
                school_id: Some(school.id),
                achievement_type: AchievementCredentialType::AcademicExcellence,
                title: "Top 1%".to_owned(),
                description: Some("Excellent term results".to_owned()),
                achievement_date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                issuance_notes: None,
                evidence_uri: Some("https://example.com/report.pdf".to_owned()),
                attestation_anchor: None,
                attestation_anchor_network: None,
                metadata: serde_json::json!({ "gpa_band": "A" }),
            },
        )
        .await
        .unwrap();

        assert_eq!(
            credential.achievement_type,
            AchievementCredentialType::AcademicExcellence
        );
        assert_eq!(credential.recipient_user_id, Some(student_id));
        assert_eq!(credential.attestation_hash.len(), 64);
    }

    #[tokio::test]
    async fn parent_can_list_credentials_for_owned_child_profiles() {
        let repos = FakeRepos::default();
        let parent_id = Uuid::new_v4();
        let student_id = Uuid::new_v4();
        let child = make_child(parent_id);
        crate::repos::ChildProfileRepository::create(&repos, child.clone()).await.unwrap();

        let saved = repos
            .create_with_audit(
                AchievementCredential {
                    id: Uuid::new_v4(),
                    credential_ref: Uuid::new_v4(),
                    child_profile_id: child.id,
                    recipient_user_id: Some(student_id),
                    school_id: None,
                    achievement_type: AchievementCredentialType::FeeFullyFunded,
                    status: AchievementCredentialStatus::Issued,
                    title: "Fee fully funded".to_owned(),
                    description: None,
                    achievement_date: NaiveDate::from_ymd_opt(2026, 3, 2).unwrap(),
                    issued_by_user_id: Uuid::new_v4(),
                    issued_by_role: UserRole::PlatformAdmin.as_str().to_owned(),
                    issuance_notes: None,
                    evidence_uri: None,
                    attestation_hash: "a".repeat(64),
                    attestation_method: "sha256".to_owned(),
                    attestation_anchor: None,
                    attestation_anchor_network: None,
                    metadata: serde_json::json!({}),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
                AuditEvent::new(
                    None,
                    crate::audit::AuditEntityType::AchievementCredential,
                    None,
                    crate::audit::AuditAction::AchievementCredentialIssued,
                    serde_json::json!({}),
                )
                .into_log(),
            )
            .await
            .unwrap();

        let items = list_credentials(
            &repos,
            &repos,
            &AuthenticatedUser {
                user_id: parent_id,
                role: UserRole::Parent,
                session_id: Uuid::new_v4(),
            },
            CredentialListFilter::default(),
        )
        .await
        .unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, saved.id);
    }
}
