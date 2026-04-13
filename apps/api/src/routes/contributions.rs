use std::sync::Arc;

use application::contributions::{
    RecordContributionIntentInput, SettlementInput, confirm_contribution, fail_contribution,
    list_vault_contributions, record_contribution_intent, reverse_contribution,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use domain::persistence::ContributionSourceType;
use serde::{Deserialize, Serialize};
use shared::currency::Currency;
use tracing::warn;
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

use super::{dto::ContributionDto, notifications::dispatcher};

#[derive(Debug, Deserialize)]
pub struct CreateContributionRequest {
    pub amount_minor: i64,
    pub currency: String,
    pub source_type: String,
    pub external_reference: Option<String>,
    pub idempotency_key: String,
}

impl Validate for CreateContributionRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.amount_minor <= 0 {
            return Err(ApiError::validation_with_field(
                "amount_minor must be > 0",
                "amount_minor",
            ));
        }
        if self.currency.trim().is_empty() {
            return Err(ApiError::validation_with_field("currency is required", "currency"));
        }
        if self.source_type.trim().is_empty() {
            return Err(ApiError::validation_with_field("source_type is required", "source_type"));
        }
        if self.idempotency_key.trim().is_empty() || self.idempotency_key.len() > 128 {
            return Err(ApiError::validation_with_field(
                "idempotency_key is required and must be <= 128 characters",
                "idempotency_key",
            ));
        }
        if self
            .external_reference
            .as_ref()
            .map(|value| value.len() > 128)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "external_reference must be <= 128 characters",
                "external_reference",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct SettleContributionRequest {
    pub external_reference: Option<String>,
}

impl Validate for SettleContributionRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self
            .external_reference
            .as_ref()
            .map(|value| value.len() > 128)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "external_reference must be <= 128 characters",
                "external_reference",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct ContributionResponseData {
    pub contribution: ContributionDto,
}

#[derive(Debug, Serialize)]
pub struct ContributionListResponseData {
    pub items: Vec<ContributionDto>,
}

pub async fn create_contribution(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<CreateContributionRequest>,
) -> Result<Json<ResponseEnvelope<ContributionResponseData>>, ApiError> {
    let currency = request
        .currency
        .parse::<Currency>()
        .map_err(|_| ApiError::validation("invalid currency supplied"))?;
    let source_type = request
        .source_type
        .parse::<ContributionSourceType>()
        .map_err(|_| ApiError::validation("invalid contribution source type supplied"))?;

    let contribution = record_contribution_intent(
        &state.vault_repo,
        &state.contribution_repo,
        &state.contribution_workflow_repo,
        &current_user.0,
        RecordContributionIntentInput {
            vault_id,
            amount_minor: request.amount_minor,
            currency,
            source_type,
            external_reference: request.external_reference,
            idempotency_key: request.idempotency_key,
        },
    )
    .await?;

    Ok(ok(ContributionResponseData { contribution: contribution.into() }))
}

pub async fn list_contributions(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<Uuid>,
    query: Query<PaginationQuery>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<ContributionListResponseData>>, ApiError> {
    let page = parse_query(query).normalize()?;
    let items = list_vault_contributions(
        &state.vault_repo,
        &state.contribution_repo,
        &current_user.0,
        vault_id,
    )
    .await?;
    let mapped = items.into_iter().map(ContributionDto::from).collect::<Vec<_>>();
    let (items, meta) = paginated(&mapped, page, None);

    Ok(ok_with_meta(ContributionListResponseData { items }, meta))
}

pub async fn confirm(
    State(state): State<Arc<AppState>>,
    Path(contribution_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<SettleContributionRequest>,
) -> Result<Json<ResponseEnvelope<ContributionResponseData>>, ApiError> {
    let contribution = confirm_contribution(
        &state.contribution_repo,
        &state.contribution_workflow_repo,
        &current_user.0,
        SettlementInput { contribution_id, external_reference: request.external_reference },
    )
    .await?;

    if let Err(error) = dispatcher(&state)
        .contribution_received_for_vault_owner(&state.vault_repo, &contribution)
        .await
    {
        warn!(%contribution_id, error = %error, "failed to dispatch contribution notification");
    }

    Ok(ok(ContributionResponseData { contribution: contribution.into() }))
}

pub async fn fail(
    State(state): State<Arc<AppState>>,
    Path(contribution_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<SettleContributionRequest>,
) -> Result<Json<ResponseEnvelope<ContributionResponseData>>, ApiError> {
    let contribution = fail_contribution(
        &state.contribution_repo,
        &state.contribution_workflow_repo,
        &current_user.0,
        SettlementInput { contribution_id, external_reference: request.external_reference },
    )
    .await?;

    Ok(ok(ContributionResponseData { contribution: contribution.into() }))
}

pub async fn reverse(
    State(state): State<Arc<AppState>>,
    Path(contribution_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<SettleContributionRequest>,
) -> Result<Json<ResponseEnvelope<ContributionResponseData>>, ApiError> {
    let contribution = reverse_contribution(
        &state.contribution_repo,
        &state.contribution_workflow_repo,
        &current_user.0,
        SettlementInput { contribution_id, external_reference: request.external_reference },
    )
    .await?;

    Ok(ok(ContributionResponseData { contribution: contribution.into() }))
}
