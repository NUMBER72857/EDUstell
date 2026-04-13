use application::repos::{AuditLogRepository, NotificationRepository};
use chrono::Utc;
use domain::persistence::{AuditLog, Notification, NotificationStatus, NotificationType};
use infrastructure::{
    persistence::repos::{AuditLogQuery, PgAuditLogRepository, PgNotificationRepository},
    testing::TestDatabase,
};
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_user(pool: &PgPool, email: &str, role: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, role, email_verified, mfa_enabled, status)
        VALUES ($1, $2, 'hashed-password', $3, FALSE, FALSE, 'active')
        "#,
    )
    .bind(id)
    .bind(email)
    .bind(role)
    .execute(pool)
    .await
    .expect("user seed should succeed");
    id
}

#[tokio::test]
async fn audit_log_repository_round_trips_by_actor_and_entity() {
    let Some(db) = TestDatabase::isolated().await.expect("db setup should not error") else {
        eprintln!("skipping repository test: TEST_DATABASE_URL or DATABASE_URL not set");
        return;
    };
    let pool = db.pool();
    let repo = PgAuditLogRepository::new(pool.clone());
    let user_id = seed_user(pool, "audit@example.com", "platform_admin").await;
    let entity_id = Uuid::new_v4();
    let now = Utc::now();

    let saved = repo
        .append(AuditLog {
            id: Uuid::new_v4(),
            actor_user_id: Some(user_id),
            entity_type: "school".to_owned(),
            entity_id: Some(entity_id),
            action: "school.verified".to_owned(),
            request_id: Some("req-test".to_owned()),
            correlation_id: Some("corr-test".to_owned()),
            metadata: serde_json::json!({ "verification_status": "verified" }),
            created_at: now,
            updated_at: now,
        })
        .await
        .expect("append should succeed");

    let by_entity =
        repo.list_by_entity("school", entity_id).await.expect("entity query should succeed");
    let by_actor = repo.list_by_actor(user_id).await.expect("actor query should succeed");
    let by_request = repo
        .query(AuditLogQuery {
            request_id: Some("req-test".to_owned()),
            correlation_id: Some("corr-test".to_owned()),
            limit: Some(10),
            ..AuditLogQuery::default()
        })
        .await
        .expect("request query should succeed");

    assert_eq!(saved.action, "school.verified");
    assert_eq!(by_entity.len(), 1);
    assert_eq!(by_actor.len(), 1);
    assert_eq!(by_request.len(), 1);
}

#[tokio::test]
async fn notification_repository_marks_read_and_unread() {
    let Some(db) = TestDatabase::isolated().await.expect("db setup should not error") else {
        eprintln!("skipping repository test: TEST_DATABASE_URL or DATABASE_URL not set");
        return;
    };
    let pool = db.pool();
    let repo = PgNotificationRepository::new(pool.clone());
    let user_id = seed_user(pool, "notify@example.com", "parent").await;
    let now = Utc::now();

    let notification = repo
        .create(Notification {
            id: Uuid::new_v4(),
            user_id,
            notification_type: NotificationType::ContributionReceived,
            title: "Contribution received".to_owned(),
            body: "A contribution was received".to_owned(),
            metadata: serde_json::json!({ "amount_minor": 5000 }),
            status: NotificationStatus::Pending,
            read_at: None,
            created_at: now,
            updated_at: now,
        })
        .await
        .expect("notification create should succeed");

    repo.mark_read(notification.id).await.expect("mark read should succeed");
    let after_read = repo
        .find_by_id(notification.id)
        .await
        .expect("find should succeed")
        .expect("notification should exist");
    assert_eq!(after_read.status, NotificationStatus::Read);

    repo.mark_unread(notification.id).await.expect("mark unread should succeed");
    let after_unread = repo
        .find_by_id(notification.id)
        .await
        .expect("find should succeed")
        .expect("notification should exist");
    assert_eq!(after_unread.status, NotificationStatus::Pending);
    assert!(after_unread.read_at.is_none());
}
