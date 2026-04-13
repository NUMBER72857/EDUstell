use chrono::Utc;
use domain::{
    auth::{AuthenticatedUser, UserRole},
    persistence::{KycProfile, KycStatus},
};
use uuid::Uuid;

use crate::{
    audit::{AuditEvent, AuditService},
    repos::{AuditLogRepository, KycRepository, PersistenceError},
};

#[derive(Debug, thiserror::Error)]
pub enum KycError {
    #[error("{0}")]
    Validation(String),
    #[error("forbidden")]
    Forbidden,
    #[error("repository error: {0}")]
    Repository(String),
}

impl From<PersistenceError> for KycError {
    fn from(value: PersistenceError) -> Self {
        match value {
            PersistenceError::NotFound => Self::Validation("profile not found".to_owned()),
            PersistenceError::Conflict(message) | PersistenceError::Validation(message) => {
                Self::Validation(message)
            }
            PersistenceError::Repository(message) => Self::Repository(message),
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateKycStatusInput {
    pub user_id: Uuid,
    pub status: KycStatus,
    pub provider_reference: Option<String>,
}

pub async fn update_kyc_status<R, A>(
    kyc: &R,
    audits: &A,
    actor: &AuthenticatedUser,
    input: UpdateKycStatusInput,
) -> Result<KycProfile, KycError>
where
    R: KycRepository,
    A: AuditLogRepository,
{
    if actor.role != UserRole::PlatformAdmin {
        return Err(KycError::Forbidden);
    }

    let existing = kyc.find_by_user(input.user_id).await?;
    let previous_status = existing.as_ref().map(|item| item.status.as_str());
    let now = Utc::now();
    let profile = KycProfile {
        id: existing.as_ref().map(|item| item.id).unwrap_or_else(Uuid::new_v4),
        user_id: input.user_id,
        status: input.status,
        provider_reference: input.provider_reference,
        reviewed_at: Some(now),
        created_at: existing.as_ref().map(|item| item.created_at).unwrap_or(now),
        updated_at: now,
    };

    let saved = kyc.upsert(profile).await?;

    if previous_status != Some(saved.status.as_str()) {
        AuditService::new(audits)
            .record(AuditEvent::kyc_status_changed(Some(actor.user_id), &saved, previous_status))
            .await
            .map_err(|error| KycError::Repository(error.to_string()))?;
    }

    Ok(saved)
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use super::*;
    use crate::repos::AuditLogRepository;

    #[derive(Default, Clone)]
    struct FakeKycRepo {
        items: Arc<Mutex<HashMap<Uuid, KycProfile>>>,
    }

    #[async_trait]
    impl KycRepository for FakeKycRepo {
        async fn upsert(&self, profile: KycProfile) -> Result<KycProfile, PersistenceError> {
            self.items.lock().await.insert(profile.user_id, profile.clone());
            Ok(profile)
        }

        async fn find_by_user(
            &self,
            user_id: Uuid,
        ) -> Result<Option<KycProfile>, PersistenceError> {
            Ok(self.items.lock().await.get(&user_id).cloned())
        }
    }

    #[derive(Default, Clone)]
    struct FakeAuditRepo {
        items: Arc<Mutex<Vec<domain::persistence::AuditLog>>>,
    }

    #[async_trait]
    impl AuditLogRepository for FakeAuditRepo {
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
    async fn kyc_status_change_records_audit_without_provider_reference() {
        let repo = FakeKycRepo::default();
        let audits = FakeAuditRepo::default();
        let actor = AuthenticatedUser {
            user_id: Uuid::new_v4(),
            role: UserRole::PlatformAdmin,
            session_id: Uuid::new_v4(),
        };

        let profile = update_kyc_status(
            &repo,
            &audits,
            &actor,
            UpdateKycStatusInput {
                user_id: Uuid::new_v4(),
                status: KycStatus::Approved,
                provider_reference: Some("provider-ref-123".to_owned()),
            },
        )
        .await
        .unwrap();

        let logged = audits.items.lock().await;
        assert_eq!(profile.status, KycStatus::Approved);
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].action, "kyc.status_changed");
        assert!(logged[0].metadata.get("provider_reference").is_none());
    }
}
