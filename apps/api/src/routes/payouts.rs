use std::sync::Arc;

use application::payouts::{
    CreatePayoutRequestInput, ReviewPayoutInput, approve_payout, complete_payout,
    create_payout_request, fail_payout, get_payout, list_vault_payouts, mark_payout_processing,
    move_payout_to_review, reject_payout,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
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

use super::{dto::PayoutDto, notifications::dispatcher};

#[derive(Debug, Deserialize)]
pub struct CreatePayoutRequestBody {
    pub milestone_id: Uuid,
    pub school_id: Uuid,
    pub amount_minor: i64,
    pub currency: String,
    pub idempotency_key: String,
}

impl Validate for CreatePayoutRequestBody {
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
        if self.idempotency_key.trim().is_empty() || self.idempotency_key.len() > 128 {
            return Err(ApiError::validation_with_field(
                "idempotency_key is required and must be <= 128 characters",
                "idempotency_key",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ReviewPayoutRequestBody {
    pub review_notes: Option<String>,
    pub external_payout_reference: Option<String>,
}

impl Validate for ReviewPayoutRequestBody {
    fn validate(&self) -> Result<(), ApiError> {
        if self
            .review_notes
            .as_ref()
            .map(|value| value.len() > 1000)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "review_notes must be <= 1000 characters",
                "review_notes",
            ));
        }
        if self
            .external_payout_reference
            .as_ref()
            .map(|value| value.len() > 128)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "external_payout_reference must be <= 128 characters",
                "external_payout_reference",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct PayoutResponseData {
    pub payout: PayoutDto,
}

#[derive(Debug, Serialize)]
pub struct PayoutListResponseData {
    pub items: Vec<PayoutDto>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<CreatePayoutRequestBody>,
) -> Result<Json<ResponseEnvelope<PayoutResponseData>>, ApiError> {
    let currency = request
        .currency
        .parse::<Currency>()
        .map_err(|_| ApiError::validation("invalid currency supplied"))?;

    let payout = create_payout_request(
        &state.vault_repo,
        &state.milestone_repo,
        &state.school_repo,
        &state.payout_repo,
        &state.payout_workflow_repo,
        &current_user.0,
        CreatePayoutRequestInput {
            vault_id,
            milestone_id: request.milestone_id,
            school_id: request.school_id,
            amount_minor: request.amount_minor,
            currency,
            idempotency_key: request.idempotency_key,
        },
    )
    .await?;

    Ok(ok(PayoutResponseData { payout: payout.into() }))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(payout_id): Path<Uuid>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<PayoutResponseData>>, ApiError> {
    let payout = get_payout(&state.payout_repo, &current_user.0, payout_id).await?;

    Ok(ok(PayoutResponseData { payout: payout.into() }))
}

pub async fn list_by_vault(
    State(state): State<Arc<AppState>>,
    Path(vault_id): Path<Uuid>,
    query: Query<PaginationQuery>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<PayoutListResponseData>>, ApiError> {
    let page = parse_query(query).normalize()?;
    let items =
        list_vault_payouts(&state.vault_repo, &state.payout_repo, &current_user.0, vault_id)
            .await?;
    let mapped = items.into_iter().map(PayoutDto::from).collect::<Vec<_>>();
    let (items, meta) = paginated(&mapped, page, None);

    Ok(ok_with_meta(PayoutListResponseData { items }, meta))
}

pub async fn move_to_review(
    State(state): State<Arc<AppState>>,
    Path(payout_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<ReviewPayoutRequestBody>,
) -> Result<Json<ResponseEnvelope<PayoutResponseData>>, ApiError> {
    let payout = move_payout_to_review(
        &state.payout_repo,
        &state.school_repo,
        &state.milestone_repo,
        &state.vault_repo,
        &state.payout_workflow_repo,
        &current_user.0,
        ReviewPayoutInput {
            payout_id,
            review_notes: request.review_notes,
            external_payout_reference: request.external_payout_reference,
        },
    )
    .await?;

    Ok(ok(PayoutResponseData { payout: payout.into() }))
}

pub async fn approve(
    State(state): State<Arc<AppState>>,
    Path(payout_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<ReviewPayoutRequestBody>,
) -> Result<Json<ResponseEnvelope<PayoutResponseData>>, ApiError> {
    let payout = approve_payout(
        &state.payout_repo,
        &state.school_repo,
        &state.milestone_repo,
        &state.vault_repo,
        &state.payout_workflow_repo,
        &current_user.0,
        ReviewPayoutInput {
            payout_id,
            review_notes: request.review_notes,
            external_payout_reference: request.external_payout_reference,
        },
    )
    .await?;

    if let Err(error) = dispatcher(&state).payout_approved(&payout).await {
        warn!(%payout_id, error = %error, "failed to dispatch payout approved notification");
    }

    Ok(ok(PayoutResponseData { payout: payout.into() }))
}

pub async fn reject(
    State(state): State<Arc<AppState>>,
    Path(payout_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<ReviewPayoutRequestBody>,
) -> Result<Json<ResponseEnvelope<PayoutResponseData>>, ApiError> {
    let payout = reject_payout(
        &state.payout_repo,
        &state.school_repo,
        &state.milestone_repo,
        &state.vault_repo,
        &state.payout_workflow_repo,
        &current_user.0,
        ReviewPayoutInput {
            payout_id,
            review_notes: request.review_notes,
            external_payout_reference: request.external_payout_reference,
        },
    )
    .await?;

    Ok(ok(PayoutResponseData { payout: payout.into() }))
}

pub async fn processing(
    State(state): State<Arc<AppState>>,
    Path(payout_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<ReviewPayoutRequestBody>,
) -> Result<Json<ResponseEnvelope<PayoutResponseData>>, ApiError> {
    let payout = mark_payout_processing(
        &state.payout_repo,
        &state.school_repo,
        &state.milestone_repo,
        &state.vault_repo,
        &state.payout_workflow_repo,
        &current_user.0,
        ReviewPayoutInput {
            payout_id,
            review_notes: request.review_notes,
            external_payout_reference: request.external_payout_reference,
        },
    )
    .await?;

    Ok(ok(PayoutResponseData { payout: payout.into() }))
}

pub async fn complete(
    State(state): State<Arc<AppState>>,
    Path(payout_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<ReviewPayoutRequestBody>,
) -> Result<Json<ResponseEnvelope<PayoutResponseData>>, ApiError> {
    let payout = complete_payout(
        &state.payout_repo,
        &state.school_repo,
        &state.milestone_repo,
        &state.vault_repo,
        &state.payout_workflow_repo,
        &current_user.0,
        ReviewPayoutInput {
            payout_id,
            review_notes: request.review_notes,
            external_payout_reference: request.external_payout_reference,
        },
    )
    .await?;

    if let Err(error) = dispatcher(&state).payout_completed(&payout).await {
        warn!(%payout_id, error = %error, "failed to dispatch payout completed notification");
    }

    Ok(ok(PayoutResponseData { payout: payout.into() }))
}

pub async fn fail(
    State(state): State<Arc<AppState>>,
    Path(payout_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<ReviewPayoutRequestBody>,
) -> Result<Json<ResponseEnvelope<PayoutResponseData>>, ApiError> {
    let payout = fail_payout(
        &state.payout_repo,
        &state.school_repo,
        &state.milestone_repo,
        &state.vault_repo,
        &state.payout_workflow_repo,
        &current_user.0,
        ReviewPayoutInput {
            payout_id,
            review_notes: request.review_notes,
            external_payout_reference: request.external_payout_reference,
        },
    )
    .await?;

    Ok(ok(PayoutResponseData { payout: payout.into() }))
}
