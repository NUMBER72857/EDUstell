use std::sync::Arc;

use application::notifications::{
    NotificationEventDispatcher, list_notification_preferences, list_notifications,
    mark_notification_read, mark_notification_unread, upsert_notification_preference,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use domain::persistence::NotificationType;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{
        PaginationQuery, ResponseEnvelope, Validate, ValidatedJson, ok, ok_with_meta, paginated,
        parse_query,
    },
    error::ApiError,
    extractors::current_user::CurrentUser,
    state::AppState,
};

use super::dto::{NotificationDto, NotificationPreferenceDto};

#[derive(Debug, Serialize)]
pub struct NotificationListResponseData {
    pub items: Vec<NotificationDto>,
}

#[derive(Debug, Serialize)]
pub struct NotificationPreferenceListResponseData {
    pub items: Vec<NotificationPreferenceDto>,
}

#[derive(Debug, Serialize)]
pub struct NotificationPreferenceResponseData {
    pub preference: NotificationPreferenceDto,
}

#[derive(Debug, Serialize)]
pub struct NotificationMutationResponseData {
    pub success: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpsertNotificationPreferenceRequest {
    pub in_app_enabled: bool,
    pub email_enabled: bool,
}

impl Validate for UpsertNotificationPreferenceRequest {
    fn validate(&self) -> Result<(), ApiError> {
        Ok(())
    }
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    query: Query<PaginationQuery>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<NotificationListResponseData>>, ApiError> {
    let page = parse_query(query).normalize()?;
    let items = list_notifications(&state.notification_repo, &current_user.0).await?;
    let mapped = items.into_iter().map(NotificationDto::from).collect::<Vec<_>>();
    let (items, meta) = paginated(&mapped, page, None);

    Ok(ok_with_meta(NotificationListResponseData { items }, meta))
}

pub async fn mark_read(
    State(state): State<Arc<AppState>>,
    Path(notification_id): Path<Uuid>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<NotificationMutationResponseData>>, ApiError> {
    mark_notification_read(&state.notification_repo, &current_user.0, notification_id).await?;

    Ok(ok(NotificationMutationResponseData { success: true }))
}

pub async fn mark_unread(
    State(state): State<Arc<AppState>>,
    Path(notification_id): Path<Uuid>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<NotificationMutationResponseData>>, ApiError> {
    mark_notification_unread(&state.notification_repo, &current_user.0, notification_id).await?;

    Ok(ok(NotificationMutationResponseData { success: true }))
}

pub async fn list_preferences(
    State(state): State<Arc<AppState>>,
    query: Query<PaginationQuery>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<NotificationPreferenceListResponseData>>, ApiError> {
    let page = parse_query(query).normalize()?;
    let items = list_notification_preferences(&state.notification_preference_repo, &current_user.0)
        .await?
        .into_iter()
        .map(NotificationPreferenceDto::from)
        .collect::<Vec<_>>();
    let (items, meta) = paginated(&items, page, None);

    Ok(ok_with_meta(NotificationPreferenceListResponseData { items }, meta))
}

pub async fn upsert_preference(
    State(state): State<Arc<AppState>>,
    Path(notification_type): Path<String>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<UpsertNotificationPreferenceRequest>,
) -> Result<Json<ResponseEnvelope<NotificationPreferenceResponseData>>, ApiError> {
    let notification_type = notification_type
        .parse::<NotificationType>()
        .map_err(|_| ApiError::validation("invalid notification type supplied"))?;
    let preference = upsert_notification_preference(
        &state.notification_preference_repo,
        &current_user.0,
        notification_type,
        request.in_app_enabled,
        request.email_enabled,
    )
    .await?;

    Ok(ok(NotificationPreferenceResponseData { preference: preference.into() }))
}

pub fn dispatcher(
    state: &AppState,
) -> NotificationEventDispatcher<
    '_,
    infrastructure::persistence::repos::PgNotificationRepository,
    infrastructure::persistence::repos::PgNotificationPreferenceRepository,
    infrastructure::notifications::NoopEmailSender,
> {
    NotificationEventDispatcher::new(
        &state.notification_repo,
        &state.notification_preference_repo,
        &state.email_sender,
    )
}
