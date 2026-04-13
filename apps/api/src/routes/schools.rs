use std::sync::Arc;

use application::schools::{
    CreateSchoolInput, VerifySchoolInput, create_school, search_verified_schools, verify_school,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use domain::persistence::{SchoolPayoutMethod, SchoolVerificationStatus};
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

use super::dto::SchoolDto;

#[derive(Debug, Deserialize)]
pub struct CreateSchoolRequest {
    pub legal_name: String,
    pub display_name: String,
    pub country: String,
    pub payout_method: String,
    pub payout_reference: String,
}

impl Validate for CreateSchoolRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.legal_name.trim().is_empty() {
            return Err(ApiError::validation_with_field("legal_name is required", "legal_name"));
        }
        if self.legal_name.len() > 200 {
            return Err(ApiError::validation_with_field(
                "legal_name must be <= 200 characters",
                "legal_name",
            ));
        }
        if self.display_name.trim().is_empty() {
            return Err(ApiError::validation_with_field(
                "display_name is required",
                "display_name",
            ));
        }
        if self.display_name.len() > 200 {
            return Err(ApiError::validation_with_field(
                "display_name must be <= 200 characters",
                "display_name",
            ));
        }
        if self.country.trim().is_empty() {
            return Err(ApiError::validation_with_field("country is required", "country"));
        }
        if self.payout_method.trim().is_empty() {
            return Err(ApiError::validation_with_field(
                "payout_method is required",
                "payout_method",
            ));
        }
        if self.payout_reference.trim().is_empty() {
            return Err(ApiError::validation_with_field(
                "payout_reference is required",
                "payout_reference",
            ));
        }
        if self.payout_reference.len() > 128 {
            return Err(ApiError::validation_with_field(
                "payout_reference must be <= 128 characters",
                "payout_reference",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct VerifySchoolRequest {
    pub verification_status: String,
}

impl Validate for VerifySchoolRequest {
    fn validate(&self) -> Result<(), ApiError> {
        if self.verification_status.trim().is_empty() {
            return Err(ApiError::validation_with_field(
                "verification_status is required",
                "verification_status",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct SchoolSearchQuery {
    pub q: Option<String>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct SchoolResponseData {
    pub school: SchoolDto,
}

#[derive(Debug, Serialize)]
pub struct SchoolListResponseData {
    pub items: Vec<SchoolDto>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<CreateSchoolRequest>,
) -> Result<Json<ResponseEnvelope<SchoolResponseData>>, ApiError> {
    let payout_method = request
        .payout_method
        .parse::<SchoolPayoutMethod>()
        .map_err(|_| ApiError::validation("invalid payout_method supplied"))?;

    let school = create_school(
        &state.school_workflow_repo,
        &current_user.0,
        CreateSchoolInput {
            legal_name: request.legal_name,
            display_name: request.display_name,
            country: request.country,
            payout_method,
            payout_reference: request.payout_reference,
        },
    )
    .await?;

    Ok(ok(SchoolResponseData { school: school.into() }))
}

pub async fn verify(
    State(state): State<Arc<AppState>>,
    Path(school_id): Path<Uuid>,
    current_user: CurrentUser,
    ValidatedJson(request): ValidatedJson<VerifySchoolRequest>,
) -> Result<Json<ResponseEnvelope<SchoolResponseData>>, ApiError> {
    let verification_status = request
        .verification_status
        .parse::<SchoolVerificationStatus>()
        .map_err(|_| ApiError::validation("invalid verification_status supplied"))?;

    let school = verify_school(
        &state.school_workflow_repo,
        &current_user.0,
        VerifySchoolInput { school_id, verification_status },
    )
    .await?;

    Ok(ok(SchoolResponseData { school: school.into() }))
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    current_user: CurrentUser,
    Query(query): Query<SchoolSearchQuery>,
) -> Result<Json<ResponseEnvelope<SchoolListResponseData>>, ApiError> {
    let page = PaginationQuery { page: query.page, per_page: query.per_page }.normalize()?;
    let items =
        search_verified_schools(&state.school_repo, &current_user.0, query.q.as_deref()).await?;
    let mapped = items.into_iter().map(SchoolDto::from).collect::<Vec<_>>();
    let (items, meta) = paginated(&mapped, page, Some(serde_json::json!({ "q": query.q })));

    Ok(ok_with_meta(SchoolListResponseData { items }, meta))
}
