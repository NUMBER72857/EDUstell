use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
};
use infrastructure::persistence::repos::AuditLogQuery;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{ResponseEnvelope, Validate, ok_with_meta},
    error::ApiError,
    extractors::current_user::CurrentUser,
    middleware::auth::require_any_role,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct AuditLogQueryParams {
    pub actor_user_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub action: Option<String>,
    pub request_id: Option<String>,
    pub correlation_id: Option<String>,
    pub limit: Option<i64>,
}

impl Validate for AuditLogQueryParams {
    fn validate(&self) -> Result<(), ApiError> {
        if let Some(limit) = self.limit {
            if !(1..=500).contains(&limit) {
                return Err(ApiError::validation_with_field("limit must be between 1 and 500", "limit"));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct AuditLogResponseData {
    pub items: Vec<domain::persistence::AuditLog>,
}

#[derive(Debug, Serialize)]
pub struct MetricsResponseData {
    pub metrics: crate::metrics::MetricsSnapshot,
}

pub async fn list_audit_logs(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<AuditLogQueryParams>,
) -> Result<Json<ResponseEnvelope<AuditLogResponseData>>, ApiError> {
    require_any_role(&current_user.0, &[domain::auth::UserRole::PlatformAdmin])?;
    query.validate()?;
    state.metrics.audit_query();

    let items = state
        .audit_repo
        .query(AuditLogQuery {
            actor_user_id: query.actor_user_id,
            entity_type: query.entity_type.clone(),
            entity_id: query.entity_id,
            action: query.action.clone(),
            request_id: query.request_id.clone(),
            correlation_id: query.correlation_id.clone(),
            limit: query.limit,
        })
        .await
        .map_err(|_| ApiError::internal("failed to query audit logs"))?;

    Ok(ok_with_meta(
        AuditLogResponseData { items },
        crate::api::Meta {
            filters: Some(serde_json::json!({
                "actor_user_id": query.actor_user_id,
                "entity_type": query.entity_type,
                "entity_id": query.entity_id,
                "action": query.action,
                "request_id": query.request_id,
                "correlation_id": query.correlation_id,
                "limit": query.limit.unwrap_or(100),
            })),
            docs: Some(crate::api::DocsMeta { version: "v1", openapi_path: "/api/openapi.json" }),
            ..crate::api::Meta::default()
        },
    ))
}

pub async fn metrics(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<MetricsResponseData>>, ApiError> {
    require_any_role(&current_user.0, &[domain::auth::UserRole::PlatformAdmin])?;

    Ok(crate::api::ok(MetricsResponseData { metrics: state.metrics.snapshot() }))
}
