use std::sync::Arc;

use application::scholarships::{
    CreateScholarshipApplicationInput, CreateScholarshipPoolInput,
    DecideScholarshipApplicationInput, FundScholarshipPoolInput, UpdateScholarshipAwardInput,
    approve_application as approve_application_service,
    create_scholarship_application as create_scholarship_application_service,
    create_scholarship_pool as create_scholarship_pool_service,
    disburse_award as disburse_award_service,
    fund_scholarship_pool as fund_scholarship_pool_service,
    get_scholarship_pool as get_scholarship_pool_service,
    list_pool_applications as list_pool_applications_service,
    list_pool_awards as list_pool_awards_service,
    list_pool_donor_contributions as list_pool_donor_contributions_service,
    list_scholarship_pools as list_scholarship_pools_service,
    reject_application as reject_application_service, revoke_award as revoke_award_service,
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

use super::{
    dto::{
        DonorContributionDto, ScholarshipApplicationDto, ScholarshipAwardDto, ScholarshipPoolDto,
    },
    notifications::dispatcher,
};

#[derive(Debug, Deserialize)]
pub struct CreateScholarshipPoolRequest {
    pub name: String,
    pub description: Option<String>,
    pub currency: String,
    pub geography_restriction: Option<String>,
    pub education_level_restriction: Option<String>,
    pub school_id_restriction: Option<Uuid>,
    pub category_restriction: Option<String>,
}

impl Validate for CreateScholarshipPoolRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.name.trim().is_empty() {
            return Err(ApiError::validation_with_field("name is required", "name"));
        }
        if self.name.len() > 120 {
            return Err(ApiError::validation_with_field("name must be <= 120 characters", "name"));
        }
        if self.currency.trim().is_empty() {
            return Err(ApiError::validation_with_field("currency is required", "currency"));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct FundScholarshipPoolRequest {
    pub amount_minor: i64,
    pub currency: String,
    pub external_reference: Option<String>,
    pub idempotency_key: String,
}

impl Validate for FundScholarshipPoolRequest {
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
pub struct CreateScholarshipApplicationRequest {
    pub child_profile_id: Uuid,
    pub student_country: Option<String>,
    pub education_level: Option<String>,
    pub school_id: Option<Uuid>,
    pub category: Option<String>,
    pub notes: Option<String>,
}

impl Validate for CreateScholarshipApplicationRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.notes.as_ref().map(|value| value.len() > 1000).unwrap_or(false) {
            return Err(ApiError::validation_with_field(
                "notes must be <= 1000 characters",
                "notes",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ScholarshipDecisionRequest {
    pub amount_minor: Option<i64>,
    pub currency: String,
    pub decision_notes: Option<String>,
}

impl Validate for ScholarshipDecisionRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.currency.trim().is_empty() {
            return Err(ApiError::validation_with_field("currency is required", "currency"));
        }
        if self
            .decision_notes
            .as_ref()
            .map(|value| value.len() > 1000)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "decision_notes must be <= 1000 characters",
                "decision_notes",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateScholarshipAwardRequest {
    pub decision_notes: Option<String>,
    pub linked_payout_request_id: Option<Uuid>,
    pub linked_vault_id: Option<Uuid>,
}

impl Validate for UpdateScholarshipAwardRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self
            .decision_notes
            .as_ref()
            .map(|value| value.len() > 1000)
            .unwrap_or(false)
        {
            return Err(ApiError::validation_with_field(
                "decision_notes must be <= 1000 characters",
                "decision_notes",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct ScholarshipPoolResponseData {
    pub pool: ScholarshipPoolDto,
}

#[derive(Debug, Serialize)]
pub struct ScholarshipPoolListResponseData {
    pub items: Vec<ScholarshipPoolDto>,
}

#[derive(Debug, Serialize)]
pub struct ScholarshipApplicationResponseData {
    pub application: ScholarshipApplicationDto,
}

#[derive(Debug, Serialize)]
pub struct ScholarshipApplicationListResponseData {
    pub items: Vec<ScholarshipApplicationDto>,
}

#[derive(Debug, Serialize)]
pub struct ScholarshipAwardListResponseData {
    pub items: Vec<ScholarshipAwardDto>,
}

#[derive(Debug, Serialize)]
pub struct DonorContributionListResponseData {
    pub items: Vec<DonorContributionDto>,
}

#[derive(Debug, Serialize)]
pub struct ScholarshipDecisionResponseData {
    pub pool: ScholarshipPoolDto,
    pub application: ScholarshipApplicationDto,
    pub award: ScholarshipAwardDto,
}

pub async fn create_pool(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<CreateScholarshipPoolRequest>,
) -> Result<Json<ResponseEnvelope<ScholarshipPoolResponseData>>, ApiError> {
    let currency = parse_currency(&request.currency)?;
    let pool = create_scholarship_pool_service(
        &state.scholarship_workflow_repo,
        &current_user.0,
        CreateScholarshipPoolInput {
            name: request.name,
            description: request.description,
            currency,
            geography_restriction: request.geography_restriction,
            education_level_restriction: request.education_level_restriction,
            school_id_restriction: request.school_id_restriction,
            category_restriction: request.category_restriction,
        },
    )
    .await?;

    Ok(ok(ScholarshipPoolResponseData { pool: pool.into() }))
}

pub async fn list_pools(
    State(state): State<Arc<AppState>>,
    query: Query<PaginationQuery>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<ScholarshipPoolListResponseData>>, ApiError> {
    let page = parse_query(query).normalize()?;
    let items =
        list_scholarship_pools_service(&state.scholarship_pool_repo, &current_user.0).await?;
    let mapped = items.into_iter().map(ScholarshipPoolDto::from).collect::<Vec<_>>();
    let (items, meta) = paginated(&mapped, page, None);
    Ok(ok_with_meta(ScholarshipPoolListResponseData { items }, meta))
}

pub async fn get_pool(
    State(state): State<Arc<AppState>>,
    Path(pool_id): Path<Uuid>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<ScholarshipPoolResponseData>>, ApiError> {
    let pool = get_scholarship_pool_service(&state.scholarship_pool_repo, &current_user.0, pool_id)
        .await?;
    Ok(ok(ScholarshipPoolResponseData { pool: pool.into() }))
}

pub async fn fund_pool(
    State(state): State<Arc<AppState>>,
    Path(pool_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<FundScholarshipPoolRequest>,
) -> Result<Json<ResponseEnvelope<ScholarshipPoolResponseData>>, ApiError> {
    let currency = parse_currency(&request.currency)?;
    let (pool, _) = fund_scholarship_pool_service(
        &state.scholarship_pool_repo,
        &state.scholarship_workflow_repo,
        &current_user.0,
        FundScholarshipPoolInput {
            pool_id,
            amount_minor: request.amount_minor,
            currency,
            external_reference: request.external_reference,
            idempotency_key: request.idempotency_key,
        },
    )
    .await?;

    Ok(ok(ScholarshipPoolResponseData { pool: pool.into() }))
}

pub async fn create_application(
    State(state): State<Arc<AppState>>,
    Path(pool_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<CreateScholarshipApplicationRequest>,
) -> Result<Json<ResponseEnvelope<ScholarshipApplicationResponseData>>, ApiError> {
    let application = create_scholarship_application_service(
        &state.scholarship_pool_repo,
        &state.child_profile_repo,
        &state.scholarship_workflow_repo,
        &current_user.0,
        CreateScholarshipApplicationInput {
            pool_id,
            child_profile_id: request.child_profile_id,
            student_country: request.student_country,
            education_level: request.education_level,
            school_id: request.school_id,
            category: request.category,
            notes: request.notes,
        },
    )
    .await?;

    Ok(ok(ScholarshipApplicationResponseData { application: application.into() }))
}

pub async fn list_applications(
    State(state): State<Arc<AppState>>,
    Path(pool_id): Path<Uuid>,
    query: Query<PaginationQuery>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<ScholarshipApplicationListResponseData>>, ApiError> {
    let page = parse_query(query).normalize()?;
    let items = list_pool_applications_service(
        &state.scholarship_application_repo,
        &current_user.0,
        pool_id,
    )
    .await?;
    let mapped = items.into_iter().map(ScholarshipApplicationDto::from).collect::<Vec<_>>();
    let (items, meta) = paginated(&mapped, page, None);
    Ok(ok_with_meta(ScholarshipApplicationListResponseData { items }, meta))
}

pub async fn list_awards(
    State(state): State<Arc<AppState>>,
    Path(pool_id): Path<Uuid>,
    query: Query<PaginationQuery>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<ScholarshipAwardListResponseData>>, ApiError> {
    let page = parse_query(query).normalize()?;
    let items =
        list_pool_awards_service(&state.scholarship_award_repo, &current_user.0, pool_id).await?;
    let mapped = items.into_iter().map(ScholarshipAwardDto::from).collect::<Vec<_>>();
    let (items, meta) = paginated(&mapped, page, None);
    Ok(ok_with_meta(ScholarshipAwardListResponseData { items }, meta))
}

pub async fn list_donor_contributions(
    State(state): State<Arc<AppState>>,
    Path(pool_id): Path<Uuid>,
    query: Query<PaginationQuery>,
    current_user: CurrentUser,
) -> Result<Json<ResponseEnvelope<DonorContributionListResponseData>>, ApiError> {
    let page = parse_query(query).normalize()?;
    let items = list_pool_donor_contributions_service(
        &state.scholarship_award_repo,
        &current_user.0,
        pool_id,
    )
    .await?;
    let mapped = items.into_iter().map(DonorContributionDto::from).collect::<Vec<_>>();
    let (items, meta) = paginated(&mapped, page, None);
    Ok(ok_with_meta(DonorContributionListResponseData { items }, meta))
}

pub async fn approve_application(
    State(state): State<Arc<AppState>>,
    Path(application_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<ScholarshipDecisionRequest>,
) -> Result<Json<ResponseEnvelope<ScholarshipDecisionResponseData>>, ApiError> {
    let currency = parse_currency(&request.currency)?;
    let decision = approve_application_service(
        &state.scholarship_pool_repo,
        &state.scholarship_application_repo,
        &state.scholarship_workflow_repo,
        &current_user.0,
        DecideScholarshipApplicationInput {
            application_id,
            amount_minor: request.amount_minor.unwrap_or_default(),
            currency,
            decision_notes: request.decision_notes,
        },
    )
    .await?;

    if let Err(error) = dispatcher(&state).scholarship_awarded(&decision).await {
        warn!(
            %application_id,
            error = %error,
            "failed to dispatch scholarship awarded notification"
        );
    }

    Ok(ok(ScholarshipDecisionResponseData {
        pool: decision.pool.into(),
        application: decision.application.into(),
        award: decision.award.into(),
    }))
}

pub async fn reject_application(
    State(state): State<Arc<AppState>>,
    Path(application_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<ScholarshipDecisionRequest>,
) -> Result<Json<ResponseEnvelope<ScholarshipDecisionResponseData>>, ApiError> {
    let currency = parse_currency(&request.currency)?;
    let decision = reject_application_service(
        &state.scholarship_pool_repo,
        &state.scholarship_application_repo,
        &state.scholarship_workflow_repo,
        &current_user.0,
        DecideScholarshipApplicationInput {
            application_id,
            amount_minor: request.amount_minor.unwrap_or_default(),
            currency,
            decision_notes: request.decision_notes,
        },
    )
    .await?;

    Ok(ok(ScholarshipDecisionResponseData {
        pool: decision.pool.into(),
        application: decision.application.into(),
        award: decision.award.into(),
    }))
}

pub async fn disburse_award(
    State(state): State<Arc<AppState>>,
    Path(award_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<UpdateScholarshipAwardRequest>,
) -> Result<Json<ResponseEnvelope<ScholarshipDecisionResponseData>>, ApiError> {
    let decision = disburse_award_service(
        &state.scholarship_pool_repo,
        &state.scholarship_application_repo,
        &state.scholarship_award_repo,
        &state.scholarship_workflow_repo,
        &current_user.0,
        UpdateScholarshipAwardInput {
            award_id,
            decision_notes: request.decision_notes,
            linked_payout_request_id: request.linked_payout_request_id,
            linked_vault_id: request.linked_vault_id,
        },
    )
    .await?;

    Ok(ok(ScholarshipDecisionResponseData {
        pool: decision.pool.into(),
        application: decision.application.into(),
        award: decision.award.into(),
    }))
}

pub async fn revoke_award(
    State(state): State<Arc<AppState>>,
    Path(award_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<UpdateScholarshipAwardRequest>,
) -> Result<Json<ResponseEnvelope<ScholarshipDecisionResponseData>>, ApiError> {
    let decision = revoke_award_service(
        &state.scholarship_pool_repo,
        &state.scholarship_application_repo,
        &state.scholarship_award_repo,
        &state.scholarship_workflow_repo,
        &current_user.0,
        UpdateScholarshipAwardInput {
            award_id,
            decision_notes: request.decision_notes,
            linked_payout_request_id: request.linked_payout_request_id,
            linked_vault_id: request.linked_vault_id,
        },
    )
    .await?;

    Ok(ok(ScholarshipDecisionResponseData {
        pool: decision.pool.into(),
        application: decision.application.into(),
        award: decision.award.into(),
    }))
}

fn parse_currency(value: &str) -> Result<Currency, ApiError> {
    value.parse::<Currency>().map_err(|_| ApiError::validation("invalid currency supplied"))
}
