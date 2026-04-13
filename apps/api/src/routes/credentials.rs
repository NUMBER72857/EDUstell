use std::sync::Arc;

use application::credentials::{
    CredentialListFilter, IssueCredentialInput, get_credential, issue_credential, list_credentials,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{
        PaginationQuery, ResponseEnvelope, Validate, ValidatedJson, ok, ok_with_meta, paginated,
    },
    error::ApiError,
    extractors::current_user::CurrentUser,
    state::AppState,
};

use super::dto::AchievementCredentialDto;

#[derive(Debug, Deserialize)]
pub struct IssueCredentialRequest {
    pub child_profile_id: Uuid,
    pub recipient_user_id: Option<Uuid>,
    pub school_id: Option<Uuid>,
    pub achievement_type: String,
    pub title: String,
    pub description: Option<String>,
    pub achievement_date: String,
    pub issuance_notes: Option<String>,
    pub evidence_uri: Option<String>,
    pub attestation_anchor: Option<String>,
    pub attestation_anchor_network: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl Validate for IssueCredentialRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.title.trim().is_empty() || self.title.len() > 140 {
            return Err(ApiError::validation_with_field(
                "title is required and must be <= 140 characters",
                "title",
            ));
        }
        if self
            .description
            .as_ref()
            .map(|value| value.len() > 2000)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "description must be <= 2000 characters",
                "description",
            ));
        }
        if self
            .issuance_notes
            .as_ref()
            .map(|value| value.len() > 2000)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "issuance_notes must be <= 2000 characters",
                "issuance_notes",
            ));
        }
        if self
            .evidence_uri
            .as_ref()
            .map(|value| value.len() > 500)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "evidence_uri must be <= 500 characters",
                "evidence_uri",
            ));
        }
        if self
            .attestation_anchor
            .as_ref()
            .map(|value| value.len() > 255)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "attestation_anchor must be <= 255 characters",
                "attestation_anchor",
            ));
        }
        if self
            .attestation_anchor_network
            .as_ref()
            .map(|value| value.len() > 64)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "attestation_anchor_network must be <= 64 characters",
                "attestation_anchor_network",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct CredentialListQuery {
    pub child_profile_id: Option<Uuid>,
    pub issued_by_me: Option<bool>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
}

impl Validate for CredentialListQuery {
    fn validate(&self) -> Result<(), ApiError> {
        PaginationQuery { page: self.page, per_page: self.per_page }.normalize()?;
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct AchievementCredentialResponseData {
    pub credential: AchievementCredentialDto,
}

#[derive(Debug, Serialize)]
pub struct AchievementCredentialListResponseData {
    pub items: Vec<AchievementCredentialDto>,
}

pub async fn issue(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<IssueCredentialRequest>,
) -> Result<Json<ResponseEnvelope<AchievementCredentialResponseData>>, ApiError> {
    let achievement_type = request
        .achievement_type
        .parse()
        .map_err(|_| ApiError::validation_with_field("invalid achievement_type", "achievement_type"))?;
    let achievement_date = NaiveDate::parse_from_str(&request.achievement_date, "%Y-%m-%d")
        .map_err(|_| ApiError::validation_with_field("achievement_date must be YYYY-MM-DD", "achievement_date"))?;

    let credential = issue_credential(
        &state.child_profile_repo,
        &state.school_repo,
        &state.achievement_credential_repo,
        &current_user.0,
        IssueCredentialInput {
            child_profile_id: request.child_profile_id,
            recipient_user_id: request.recipient_user_id,
            school_id: request.school_id,
            achievement_type,
            title: request.title,
            description: request.description,
            achievement_date,
            issuance_notes: request.issuance_notes,
            evidence_uri: request.evidence_uri,
            attestation_anchor: request.attestation_anchor,
            attestation_anchor_network: request.attestation_anchor_network,
            metadata: request.metadata.unwrap_or_else(|| serde_json::json!({})),
        },
    )
    .await?;

    Ok(ok(AchievementCredentialResponseData { credential: credential.into() }))
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<CredentialListQuery>,
) -> Result<Json<ResponseEnvelope<AchievementCredentialListResponseData>>, ApiError> {
    query.validate()?;
    let page = PaginationQuery { page: query.page, per_page: query.per_page }.normalize()?;
    let items = list_credentials(
        &state.child_profile_repo,
        &state.achievement_credential_repo,
        &current_user.0,
        CredentialListFilter {
            child_profile_id: query.child_profile_id,
            issued_by_me: query.issued_by_me.unwrap_or(false),
        },
    )
    .await?;
    let mapped = items.into_iter().map(AchievementCredentialDto::from).collect::<Vec<_>>();
    let (items, meta) = paginated(
        &mapped,
        page,
        Some(serde_json::json!({
            "child_profile_id": query.child_profile_id,
            "issued_by_me": query.issued_by_me.unwrap_or(false),
        })),
    );

    Ok(ok_with_meta(AchievementCredentialListResponseData { items }, meta))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(credential_id): Path<Uuid>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<AchievementCredentialResponseData>>, ApiError> {
    let credential = get_credential(
        &state.child_profile_repo,
        &state.achievement_credential_repo,
        &current_user.0,
        credential_id,
    )
    .await?;

    Ok(ok(AchievementCredentialResponseData { credential: credential.into() }))
}
