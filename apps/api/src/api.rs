use axum::{
    Json,
    extract::{FromRequest, Query, Request, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::error::ApiError;

#[derive(Debug, Serialize)]
pub struct ResponseEnvelope<T> {
    pub data: T,
    pub meta: Meta,
}

#[derive(Debug, Default, Serialize)]
pub struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pagination: Option<PaginationMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<DocsMeta>,
}

#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub page: usize,
    pub per_page: usize,
    pub total_items: usize,
    pub total_pages: usize,
}

#[derive(Debug, Serialize)]
pub struct AuthMeta {
    pub required: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DocsMeta {
    pub version: &'static str,
    pub openapi_path: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<usize>,
    pub per_page: Option<usize>,
}

impl PaginationQuery {
    pub fn normalize(&self) -> Result<Page, ApiError> {
        let page = self.page.unwrap_or(1);
        let per_page = self.per_page.unwrap_or(20);
        if page == 0 {
            return Err(ApiError::validation_with_field("page must be >= 1", "page"));
        }
        if per_page == 0 || per_page > 100 {
            return Err(ApiError::validation_with_field(
                "per_page must be between 1 and 100",
                "per_page",
            ));
        }
        Ok(Page { page, per_page })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub page: usize,
    pub per_page: usize,
}

pub fn ok<T>(data: T) -> Json<ResponseEnvelope<T>> {
    Json(ResponseEnvelope {
        data,
        meta: Meta {
            docs: Some(DocsMeta { version: "v1", openapi_path: "/api/openapi.json" }),
            ..Meta::default()
        },
    })
}

pub fn ok_with_meta<T>(data: T, meta: Meta) -> Json<ResponseEnvelope<T>> {
    Json(ResponseEnvelope { data, meta })
}

pub fn paginated<T: Clone>(
    items: &[T],
    page: Page,
    filters: Option<serde_json::Value>,
) -> (Vec<T>, Meta) {
    let total_items = items.len();
    let total_pages = if total_items == 0 { 0 } else { total_items.div_ceil(page.per_page) };
    let start = page.per_page.saturating_mul(page.page.saturating_sub(1));
    let paged = items.iter().skip(start).take(page.per_page).cloned().collect::<Vec<_>>();

    (
        paged,
        Meta {
            pagination: Some(PaginationMeta {
                page: page.page,
                per_page: page.per_page,
                total_items,
                total_pages,
            }),
            filters,
            docs: Some(DocsMeta { version: "v1", openapi_path: "/api/openapi.json" }),
            ..Meta::default()
        },
    )
}

pub trait Validate {
    fn validate(&self) -> Result<(), ApiError>;
}

pub struct ValidatedJson<T>(pub T);

impl<S, T> FromRequest<S> for ValidatedJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Validate,
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state).await.map_err(map_json_rejection)?;
        value.validate()?;
        Ok(Self(value))
    }
}

fn map_json_rejection(rejection: JsonRejection) -> ApiError {
    ApiError::validation(rejection.body_text())
}

pub fn parse_query<Q>(query: Query<Q>) -> Q {
    query.0
}

pub fn docs_response() -> Response {
    (StatusCode::OK, [("content-type", "text/markdown; charset=utf-8")], api_docs_markdown())
        .into_response()
}

pub fn openapi_response() -> Json<serde_json::Value> {
    Json(openapi_spec())
}

fn api_docs_markdown() -> &'static str {
    r#"# EDUstell API

Base URL: `/api/v1`

Auth:
- Public: `POST /auth/register`, `POST /auth/login`, `POST /auth/refresh`, `POST /auth/verify-email`
- Bearer token required for all other `/api/v1` endpoints
- Platform admin only: school verification and payout review/admin endpoints
- Achievement credentials: issue/list/get plus lightweight HTML pages at `/credentials` and `/issuer/credentials`

Patterns:
- Success envelope: `{ "data": ..., "meta": { ... } }`
- Error envelope: `{ "error": { "code", "message", "details?", "request_id?" } }`
- Pagination query: `?page=1&per_page=20`
- Filtering: query params echoed in `meta.filters`

Docs:
- OpenAPI JSON: `/api/openapi.json`
- Observability notes: `docs/observability.md`
"#
}

fn openapi_spec() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "EDUstell API",
            "version": "v1",
            "description": "Versioned JSON API for EDUstell web/mobile clients."
        },
        "servers": [{ "url": "/api/v1" }],
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "JWT"
                }
            },
            "schemas": {
                "ResponseEnvelope": {
                    "type": "object",
                    "properties": {
                        "data": {},
                        "meta": { "type": "object" }
                    },
                    "required": ["data", "meta"]
                },
                "ErrorEnvelope": {
                    "type": "object",
                    "properties": {
                        "error": {
                            "type": "object",
                            "properties": {
                                "code": { "type": "string" },
                                "message": { "type": "string" },
                                "request_id": { "type": "string" },
                                "details": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "field": { "type": ["string", "null"] },
                                            "message": { "type": "string" }
                                        }
                                    }
                                }
                            },
                            "required": ["code", "message"]
                        }
                    },
                    "required": ["error"]
                }
            }
        },
        "paths": {
            "/auth/register": {
                "post": {
                    "summary": "Register user",
                    "security": [],
                    "responses": { "200": { "description": "Registered" } }
                }
            },
            "/auth/login": {
                "post": {
                    "summary": "Login",
                    "security": [],
                    "responses": { "200": { "description": "Logged in" } }
                }
            },
            "/notifications": {
                "get": {
                    "summary": "List notifications",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "page", "in": "query", "schema": { "type": "integer", "minimum": 1 } },
                        { "name": "per_page", "in": "query", "schema": { "type": "integer", "minimum": 1, "maximum": 100 } }
                    ],
                    "responses": { "200": { "description": "Notification list" } }
                }
            },
            "/credentials": {
                "get": {
                    "summary": "List visible achievement credentials",
                    "security": [{ "bearerAuth": [] }],
                    "responses": { "200": { "description": "Credential list" } }
                },
                "post": {
                    "summary": "Issue an achievement credential",
                    "security": [{ "bearerAuth": [] }],
                    "responses": { "200": { "description": "Credential issued" } }
                }
            },
            "/credentials/{id}": {
                "get": {
                    "summary": "Get achievement credential",
                    "security": [{ "bearerAuth": [] }],
                    "parameters": [
                        { "name": "id", "in": "path", "required": true, "schema": { "type": "string", "format": "uuid" } }
                    ],
                    "responses": { "200": { "description": "Credential detail" } }
                }
            },
            "/admin/audit-logs": {
                "get": {
                    "summary": "Inspect audit logs",
                    "security": [{ "bearerAuth": [] }],
                    "responses": { "200": { "description": "Audit log list" } }
                }
            },
            "/internal/metrics": {
                "get": {
                    "summary": "Read internal metrics snapshot",
                    "security": [{ "bearerAuth": [] }],
                    "responses": { "200": { "description": "Metrics snapshot" } }
                }
            }
        }
    })
}
