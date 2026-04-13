use chrono::Utc;
use domain::{
    auth::AuthenticatedUser,
    persistence::{
        Contribution, Notification, NotificationPreference, NotificationStatus, NotificationType,
        PayoutRequest,
    },
};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    repos::{
        NotificationPreferenceRepository, NotificationRepository, PersistenceError, VaultRepository,
    },
    scholarships::ScholarshipReviewDecision,
};

#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("{0}")]
    Validation(String),
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("repository error: {0}")]
    Repository(String),
}

impl From<PersistenceError> for NotificationError {
    fn from(value: PersistenceError) -> Self {
        match value {
            PersistenceError::NotFound => Self::NotFound,
            PersistenceError::Conflict(message) | PersistenceError::Validation(message) => {
                Self::Validation(message)
            }
            PersistenceError::Repository(message) => Self::Repository(message),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmailMessage {
    pub to_user_id: Uuid,
    pub notification_type: NotificationType,
    pub subject: String,
    pub body: String,
    pub metadata: Value,
}

pub trait EmailSender: Send + Sync {
    fn send(&self, message: EmailMessage) -> Result<(), NotificationError>;
}

pub struct NotificationEventDispatcher<'a, R, P, E> {
    notifications: &'a R,
    preferences: &'a P,
    email_sender: &'a E,
}

impl<'a, R, P, E> NotificationEventDispatcher<'a, R, P, E>
where
    R: NotificationRepository,
    P: NotificationPreferenceRepository,
    E: EmailSender,
{
    pub fn new(notifications: &'a R, preferences: &'a P, email_sender: &'a E) -> Self {
        Self { notifications, preferences, email_sender }
    }

    pub async fn dispatch(
        &self,
        event: ApplicationEvent,
    ) -> Result<Option<Notification>, NotificationError> {
        handle_application_event(self.notifications, self.preferences, self.email_sender, event)
            .await
    }

    pub async fn contribution_received_for_vault_owner<V>(
        &self,
        vaults: &V,
        contribution: &Contribution,
    ) -> Result<Option<Notification>, NotificationError>
    where
        V: VaultRepository,
    {
        let vault =
            vaults.find_by_id(contribution.vault_id).await?.ok_or(NotificationError::NotFound)?;

        self.dispatch(ApplicationEvent::ContributionReceived {
            recipient_user_id: vault.owner_user_id,
            contribution_id: contribution.id,
            vault_id: contribution.vault_id,
            amount_minor: contribution.amount.amount_minor,
            currency: contribution.amount.currency.as_str().to_owned(),
        })
        .await
    }

    pub async fn payout_approved(
        &self,
        payout: &PayoutRequest,
    ) -> Result<Option<Notification>, NotificationError> {
        self.dispatch(ApplicationEvent::PayoutApproved {
            recipient_user_id: payout.requested_by,
            payout_id: payout.id,
            vault_id: payout.vault_id,
            amount_minor: payout.amount.amount_minor,
            currency: payout.amount.currency.as_str().to_owned(),
        })
        .await
    }

    pub async fn payout_completed(
        &self,
        payout: &PayoutRequest,
    ) -> Result<Option<Notification>, NotificationError> {
        self.dispatch(ApplicationEvent::PayoutCompleted {
            recipient_user_id: payout.requested_by,
            payout_id: payout.id,
            vault_id: payout.vault_id,
            amount_minor: payout.amount.amount_minor,
            currency: payout.amount.currency.as_str().to_owned(),
        })
        .await
    }

    pub async fn scholarship_awarded(
        &self,
        decision: &ScholarshipReviewDecision,
    ) -> Result<Option<Notification>, NotificationError> {
        self.dispatch(ApplicationEvent::ScholarshipAwarded {
            recipient_user_id: decision.application.applicant_user_id,
            award_id: decision.award.id,
            application_id: decision.application.id,
            amount_minor: decision.award.amount.amount_minor,
            currency: decision.award.amount.currency.as_str().to_owned(),
        })
        .await
    }

    pub async fn milestone_due_soon(
        &self,
        recipient_user_id: Uuid,
        milestone_id: Uuid,
        vault_id: Uuid,
        due_date: String,
        title: String,
    ) -> Result<Option<Notification>, NotificationError> {
        self.dispatch(ApplicationEvent::MilestoneDueSoon {
            recipient_user_id,
            milestone_id,
            vault_id,
            due_date,
            title,
        })
        .await
    }

    pub async fn milestone_underfunded(
        &self,
        recipient_user_id: Uuid,
        milestone_id: Uuid,
        vault_id: Uuid,
        shortfall_minor: i64,
        currency: String,
        title: String,
    ) -> Result<Option<Notification>, NotificationError> {
        self.dispatch(ApplicationEvent::MilestoneUnderfunded {
            recipient_user_id,
            milestone_id,
            vault_id,
            shortfall_minor,
            currency,
            title,
        })
        .await
    }

    pub async fn kyc_action_required(
        &self,
        recipient_user_id: Uuid,
        reason: String,
    ) -> Result<Option<Notification>, NotificationError> {
        self.dispatch(ApplicationEvent::KycActionRequired { recipient_user_id, reason }).await
    }
}

#[derive(Debug, Clone)]
pub enum ApplicationEvent {
    ContributionReceived {
        recipient_user_id: Uuid,
        contribution_id: Uuid,
        vault_id: Uuid,
        amount_minor: i64,
        currency: String,
    },
    MilestoneDueSoon {
        recipient_user_id: Uuid,
        milestone_id: Uuid,
        vault_id: Uuid,
        due_date: String,
        title: String,
    },
    MilestoneUnderfunded {
        recipient_user_id: Uuid,
        milestone_id: Uuid,
        vault_id: Uuid,
        shortfall_minor: i64,
        currency: String,
        title: String,
    },
    PayoutApproved {
        recipient_user_id: Uuid,
        payout_id: Uuid,
        vault_id: Uuid,
        amount_minor: i64,
        currency: String,
    },
    PayoutCompleted {
        recipient_user_id: Uuid,
        payout_id: Uuid,
        vault_id: Uuid,
        amount_minor: i64,
        currency: String,
    },
    ScholarshipAwarded {
        recipient_user_id: Uuid,
        award_id: Uuid,
        application_id: Uuid,
        amount_minor: i64,
        currency: String,
    },
    KycActionRequired {
        recipient_user_id: Uuid,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct RenderedNotificationTemplate {
    pub notification_type: NotificationType,
    pub title: String,
    pub body: String,
    pub metadata: Value,
}

pub fn render_notification_template(event: &ApplicationEvent) -> RenderedNotificationTemplate {
    match event {
        ApplicationEvent::ContributionReceived {
            contribution_id,
            vault_id,
            amount_minor,
            currency,
            ..
        } => RenderedNotificationTemplate {
            notification_type: NotificationType::ContributionReceived,
            title: "Contribution received".to_owned(),
            body: format!("A contribution of {amount_minor} {currency} was received."),
            metadata: json!({
                "contribution_id": contribution_id,
                "vault_id": vault_id,
                "amount_minor": amount_minor,
                "currency": currency,
            }),
        },
        ApplicationEvent::MilestoneDueSoon { milestone_id, vault_id, due_date, title, .. } => {
            RenderedNotificationTemplate {
                notification_type: NotificationType::MilestoneDueSoon,
                title: "Milestone due soon".to_owned(),
                body: format!("Milestone \"{title}\" is due on {due_date}."),
                metadata: json!({
                    "milestone_id": milestone_id,
                    "vault_id": vault_id,
                    "due_date": due_date,
                    "milestone_title": title,
                }),
            }
        }
        ApplicationEvent::MilestoneUnderfunded {
            milestone_id,
            vault_id,
            shortfall_minor,
            currency,
            title,
            ..
        } => RenderedNotificationTemplate {
            notification_type: NotificationType::MilestoneUnderfunded,
            title: "Milestone underfunded".to_owned(),
            body: format!("Milestone \"{title}\" is short by {shortfall_minor} {currency}."),
            metadata: json!({
                "milestone_id": milestone_id,
                "vault_id": vault_id,
                "shortfall_minor": shortfall_minor,
                "currency": currency,
                "milestone_title": title,
            }),
        },
        ApplicationEvent::PayoutApproved {
            payout_id, vault_id, amount_minor, currency, ..
        } => RenderedNotificationTemplate {
            notification_type: NotificationType::PayoutApproved,
            title: "Payout approved".to_owned(),
            body: format!("Payout of {amount_minor} {currency} has been approved."),
            metadata: json!({
                "payout_id": payout_id,
                "vault_id": vault_id,
                "amount_minor": amount_minor,
                "currency": currency,
            }),
        },
        ApplicationEvent::PayoutCompleted {
            payout_id, vault_id, amount_minor, currency, ..
        } => RenderedNotificationTemplate {
            notification_type: NotificationType::PayoutCompleted,
            title: "Payout completed".to_owned(),
            body: format!("Payout of {amount_minor} {currency} has completed."),
            metadata: json!({
                "payout_id": payout_id,
                "vault_id": vault_id,
                "amount_minor": amount_minor,
                "currency": currency,
            }),
        },
        ApplicationEvent::ScholarshipAwarded {
            award_id,
            application_id,
            amount_minor,
            currency,
            ..
        } => RenderedNotificationTemplate {
            notification_type: NotificationType::ScholarshipAwarded,
            title: "Scholarship awarded".to_owned(),
            body: format!("A scholarship award of {amount_minor} {currency} has been approved."),
            metadata: json!({
                "award_id": award_id,
                "application_id": application_id,
                "amount_minor": amount_minor,
                "currency": currency,
            }),
        },
        ApplicationEvent::KycActionRequired { reason, .. } => RenderedNotificationTemplate {
            notification_type: NotificationType::KycActionRequired,
            title: "KYC action required".to_owned(),
            body: reason.clone(),
            metadata: json!({ "reason": reason }),
        },
    }
}

pub async fn handle_application_event<R, P, E>(
    notifications: &R,
    preferences: &P,
    email_sender: &E,
    event: ApplicationEvent,
) -> Result<Option<Notification>, NotificationError>
where
    R: NotificationRepository,
    P: NotificationPreferenceRepository,
    E: EmailSender,
{
    let recipient_user_id = event_recipient(&event);
    let rendered = render_notification_template(&event);
    let preference = preferences
        .find_by_user_and_type(recipient_user_id, rendered.notification_type.as_str())
        .await?;
    let preference =
        preference.unwrap_or(default_preference(recipient_user_id, rendered.notification_type));

    let mut stored = None;
    if preference.in_app_enabled {
        let now = Utc::now();
        stored = Some(
            notifications
                .create(Notification {
                    id: Uuid::new_v4(),
                    user_id: recipient_user_id,
                    notification_type: rendered.notification_type,
                    title: rendered.title.clone(),
                    body: rendered.body.clone(),
                    metadata: rendered.metadata.clone(),
                    status: NotificationStatus::Pending,
                    read_at: None,
                    created_at: now,
                    updated_at: now,
                })
                .await?,
        );
    }

    if preference.email_enabled {
        email_sender.send(EmailMessage {
            to_user_id: recipient_user_id,
            notification_type: rendered.notification_type,
            subject: rendered.title,
            body: rendered.body,
            metadata: rendered.metadata,
        })?;
    }

    Ok(stored)
}

pub async fn list_notifications<R>(
    notifications: &R,
    actor: &AuthenticatedUser,
) -> Result<Vec<Notification>, NotificationError>
where
    R: NotificationRepository,
{
    notifications.list_by_user(actor.user_id).await.map_err(Into::into)
}

pub async fn mark_notification_read<R>(
    notifications: &R,
    actor: &AuthenticatedUser,
    notification_id: Uuid,
) -> Result<(), NotificationError>
where
    R: NotificationRepository,
{
    let notification =
        notifications.find_by_id(notification_id).await?.ok_or(NotificationError::NotFound)?;
    if notification.user_id != actor.user_id {
        return Err(NotificationError::Forbidden);
    }
    notifications.mark_read(notification_id).await?;
    Ok(())
}

pub async fn mark_notification_unread<R>(
    notifications: &R,
    actor: &AuthenticatedUser,
    notification_id: Uuid,
) -> Result<(), NotificationError>
where
    R: NotificationRepository,
{
    let notification =
        notifications.find_by_id(notification_id).await?.ok_or(NotificationError::NotFound)?;
    if notification.user_id != actor.user_id {
        return Err(NotificationError::Forbidden);
    }
    notifications.mark_unread(notification_id).await?;
    Ok(())
}

pub async fn list_notification_preferences<R>(
    preferences: &R,
    actor: &AuthenticatedUser,
) -> Result<Vec<NotificationPreference>, NotificationError>
where
    R: NotificationPreferenceRepository,
{
    preferences.list_by_user(actor.user_id).await.map_err(Into::into)
}

pub async fn upsert_notification_preference<R>(
    preferences: &R,
    actor: &AuthenticatedUser,
    notification_type: NotificationType,
    in_app_enabled: bool,
    email_enabled: bool,
) -> Result<NotificationPreference, NotificationError>
where
    R: NotificationPreferenceRepository,
{
    let now = Utc::now();
    let existing =
        preferences.find_by_user_and_type(actor.user_id, notification_type.as_str()).await?;

    let preference = NotificationPreference {
        id: existing.as_ref().map(|item| item.id).unwrap_or_else(Uuid::new_v4),
        user_id: actor.user_id,
        notification_type,
        in_app_enabled,
        email_enabled,
        created_at: existing.as_ref().map(|item| item.created_at).unwrap_or(now),
        updated_at: now,
    };

    preferences.upsert(preference).await.map_err(Into::into)
}

fn default_preference(
    user_id: Uuid,
    notification_type: NotificationType,
) -> NotificationPreference {
    let now = Utc::now();
    NotificationPreference {
        id: Uuid::new_v4(),
        user_id,
        notification_type,
        in_app_enabled: true,
        email_enabled: false,
        created_at: now,
        updated_at: now,
    }
}

fn event_recipient(event: &ApplicationEvent) -> Uuid {
    match event {
        ApplicationEvent::ContributionReceived { recipient_user_id, .. }
        | ApplicationEvent::MilestoneDueSoon { recipient_user_id, .. }
        | ApplicationEvent::MilestoneUnderfunded { recipient_user_id, .. }
        | ApplicationEvent::PayoutApproved { recipient_user_id, .. }
        | ApplicationEvent::PayoutCompleted { recipient_user_id, .. }
        | ApplicationEvent::ScholarshipAwarded { recipient_user_id, .. }
        | ApplicationEvent::KycActionRequired { recipient_user_id, .. } => *recipient_user_id,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use domain::{
        auth::AuthenticatedUser,
        persistence::{ContributionStatus, NotificationStatus, NotificationType, SavingsVault},
    };
    use shared::{currency::Currency, money::Money};

    use super::*;
    use crate::repos::VaultRepository;

    #[derive(Default, Clone)]
    struct FakeNotifications {
        items: Arc<tokio::sync::Mutex<HashMap<Uuid, Notification>>>,
    }

    #[async_trait]
    impl NotificationRepository for FakeNotifications {
        async fn create(
            &self,
            notification: Notification,
        ) -> Result<Notification, PersistenceError> {
            self.items.lock().await.insert(notification.id, notification.clone());
            Ok(notification)
        }
        async fn find_by_id(&self, id: Uuid) -> Result<Option<Notification>, PersistenceError> {
            Ok(self.items.lock().await.get(&id).cloned())
        }
        async fn list_by_user(&self, user_id: Uuid) -> Result<Vec<Notification>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .await
                .values()
                .filter(|item| item.user_id == user_id)
                .cloned()
                .collect())
        }
        async fn mark_read(&self, id: Uuid) -> Result<(), PersistenceError> {
            if let Some(item) = self.items.lock().await.get_mut(&id) {
                item.status = NotificationStatus::Read;
                item.read_at = Some(Utc::now());
            }
            Ok(())
        }
        async fn mark_unread(&self, id: Uuid) -> Result<(), PersistenceError> {
            if let Some(item) = self.items.lock().await.get_mut(&id) {
                item.status = NotificationStatus::Pending;
                item.read_at = None;
            }
            Ok(())
        }
    }

    #[derive(Default, Clone)]
    struct FakeVaults {
        items: Arc<tokio::sync::Mutex<HashMap<Uuid, SavingsVault>>>,
        contributors:
            Arc<tokio::sync::Mutex<HashMap<Uuid, Vec<domain::persistence::VaultContributor>>>>,
    }

    #[async_trait]
    impl VaultRepository for FakeVaults {
        async fn create(&self, vault: SavingsVault) -> Result<SavingsVault, PersistenceError> {
            self.items.lock().await.insert(vault.id, vault.clone());
            Ok(vault)
        }

        async fn find_by_id(&self, id: Uuid) -> Result<Option<SavingsVault>, PersistenceError> {
            Ok(self.items.lock().await.get(&id).cloned())
        }

        async fn update_balances(
            &self,
            _id: Uuid,
            _total_contributed_minor: i64,
            _total_locked_minor: i64,
            _expected_version: i64,
        ) -> Result<(), PersistenceError> {
            Ok(())
        }

        async fn add_contributor(
            &self,
            contributor: domain::persistence::VaultContributor,
        ) -> Result<domain::persistence::VaultContributor, PersistenceError> {
            self.contributors
                .lock()
                .await
                .entry(contributor.vault_id)
                .or_default()
                .push(contributor.clone());
            Ok(contributor)
        }

        async fn list_contributors(
            &self,
            vault_id: Uuid,
        ) -> Result<Vec<domain::persistence::VaultContributor>, PersistenceError> {
            Ok(self.contributors.lock().await.get(&vault_id).cloned().unwrap_or_default())
        }
    }

    #[derive(Default, Clone)]
    struct FakePreferences {
        items: Arc<tokio::sync::Mutex<HashMap<(Uuid, String), NotificationPreference>>>,
    }

    #[async_trait]
    impl NotificationPreferenceRepository for FakePreferences {
        async fn upsert(
            &self,
            preference: NotificationPreference,
        ) -> Result<NotificationPreference, PersistenceError> {
            self.items.lock().await.insert(
                (preference.user_id, preference.notification_type.as_str().to_owned()),
                preference.clone(),
            );
            Ok(preference)
        }
        async fn find_by_user_and_type(
            &self,
            user_id: Uuid,
            notification_type: &str,
        ) -> Result<Option<NotificationPreference>, PersistenceError> {
            Ok(self.items.lock().await.get(&(user_id, notification_type.to_owned())).cloned())
        }
        async fn list_by_user(
            &self,
            user_id: Uuid,
        ) -> Result<Vec<NotificationPreference>, PersistenceError> {
            Ok(self
                .items
                .lock()
                .await
                .values()
                .filter(|item| item.user_id == user_id)
                .cloned()
                .collect())
        }
    }

    #[derive(Default)]
    struct FakeEmailSender {
        sent: Mutex<Vec<EmailMessage>>,
    }

    impl EmailSender for FakeEmailSender {
        fn send(&self, message: EmailMessage) -> Result<(), NotificationError> {
            self.sent.lock().unwrap().push(message);
            Ok(())
        }
    }

    #[tokio::test]
    async fn application_event_creates_in_app_notification() {
        let notifications = FakeNotifications::default();
        let preferences = FakePreferences::default();
        let email = FakeEmailSender::default();
        let user_id = Uuid::new_v4();

        let stored = handle_application_event(
            &notifications,
            &preferences,
            &email,
            ApplicationEvent::ContributionReceived {
                recipient_user_id: user_id,
                contribution_id: Uuid::new_v4(),
                vault_id: Uuid::new_v4(),
                amount_minor: 5000,
                currency: "USD".to_owned(),
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(stored.notification_type, NotificationType::ContributionReceived);
    }

    #[tokio::test]
    async fn email_preference_triggers_email_send() {
        let notifications = FakeNotifications::default();
        let preferences = FakePreferences::default();
        let email = FakeEmailSender::default();
        let user_id = Uuid::new_v4();
        preferences
            .upsert(NotificationPreference {
                id: Uuid::new_v4(),
                user_id,
                notification_type: NotificationType::KycActionRequired,
                in_app_enabled: false,
                email_enabled: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await
            .unwrap();

        let stored = handle_application_event(
            &notifications,
            &preferences,
            &email,
            ApplicationEvent::KycActionRequired {
                recipient_user_id: user_id,
                reason: "Upload ID".to_owned(),
            },
        )
        .await
        .unwrap();

        assert!(stored.is_none());
        assert_eq!(email.sent.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn mark_read_and_unread_requires_owner() {
        let notifications = FakeNotifications::default();
        let user_id = Uuid::new_v4();
        let notification = notifications
            .create(Notification {
                id: Uuid::new_v4(),
                user_id,
                notification_type: NotificationType::PayoutApproved,
                title: "t".to_owned(),
                body: "b".to_owned(),
                metadata: json!({}),
                status: NotificationStatus::Pending,
                read_at: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await
            .unwrap();
        let actor = AuthenticatedUser {
            user_id,
            role: domain::auth::UserRole::Parent,
            session_id: Uuid::new_v4(),
        };

        mark_notification_read(&notifications, &actor, notification.id).await.unwrap();
        mark_notification_unread(&notifications, &actor, notification.id).await.unwrap();

        let stored = notifications.find_by_id(notification.id).await.unwrap().unwrap();
        assert_eq!(stored.status, NotificationStatus::Pending);
    }

    #[tokio::test]
    async fn contribution_helper_routes_notification_to_vault_owner() {
        let notifications = FakeNotifications::default();
        let preferences = FakePreferences::default();
        let email = FakeEmailSender::default();
        let vaults = FakeVaults::default();
        let owner_id = Uuid::new_v4();
        let vault_id = Uuid::new_v4();

        vaults
            .create(SavingsVault {
                id: vault_id,
                plan_id: Uuid::new_v4(),
                owner_user_id: owner_id,
                currency: "USD".to_owned(),
                status: domain::persistence::VaultStatus::Active,
                total_contributed_minor: 0,
                total_locked_minor: 0,
                total_disbursed_minor: 0,
                external_wallet_account_id: None,
                external_contract_ref: None,
                version: 0,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
            .await
            .unwrap();

        let contribution = Contribution {
            id: Uuid::new_v4(),
            vault_id,
            contributor_user_id: Uuid::new_v4(),
            amount: Money::new(5_000, Currency::Fiat("USD".to_owned())).unwrap(),
            status: ContributionStatus::Confirmed,
            source_type: domain::persistence::ContributionSourceType::Manual,
            external_reference: None,
            idempotency_key: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let dispatcher = NotificationEventDispatcher::new(&notifications, &preferences, &email);
        let stored = dispatcher
            .contribution_received_for_vault_owner(&vaults, &contribution)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.user_id, owner_id);
        assert_eq!(stored.notification_type, NotificationType::ContributionReceived);
    }
}
