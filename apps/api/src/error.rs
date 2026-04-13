use application::auth::AuthError;
use application::contributions::ContributionError;
use application::credentials::CredentialError;
use application::notifications::NotificationError;
use application::payouts::PayoutError;
use application::scholarships::ScholarshipError;
use application::schools::SchoolError;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use infrastructure::db::migrations::InfrastructureError;
use serde::Serialize;
use shared::error_codes::ErrorCode;

#[derive(Debug, thiserror::Error)]
pub enum InternalError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("startup error: {0}")]
    Startup(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("migration error")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("io error")]
    Io(#[source] std::io::Error),
    #[error("infrastructure error")]
    Infrastructure(#[from] InfrastructureError),
    #[error("auth error")]
    Auth(#[from] AuthError),
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: ErrorCode,
    message: String,
    details: Vec<ErrorDetail>,
}

impl ApiError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: ErrorCode::Validation,
            message: message.into(),
            details: vec![],
        }
    }

    pub fn validation_with_field(message: impl Into<String>, field: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            status: StatusCode::BAD_REQUEST,
            code: ErrorCode::Validation,
            message: message.clone(),
            details: vec![ErrorDetail { field: Some(field.into()), message }],
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: ErrorCode::Internal,
            message: message.into(),
            details: vec![],
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    details: Vec<ErrorDetail>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    field: Option<String>,
    message: String,
}

impl ErrorEnvelope {
    pub fn internal(request_id: Option<String>) -> Self {
        Self {
            error: ErrorBody {
                code: ErrorCode::Internal.as_str(),
                message: "internal server error".to_owned(),
                details: vec![],
                request_id,
            },
        }
    }
}

impl From<InternalError> for ApiError {
    fn from(value: InternalError) -> Self {
        match value {
            InternalError::Config(message) | InternalError::Startup(message) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: ErrorCode::Internal,
                message,
                details: vec![],
            },
            InternalError::Auth(auth_err) => Self::from(auth_err),
            InternalError::Database(_)
            | InternalError::Migration(_)
            | InternalError::Io(_)
            | InternalError::Infrastructure(_) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: ErrorCode::InfrastructureUnavailable,
                message: "internal server error".to_owned(),
                details: vec![],
            },
        }
    }
}

impl From<AuthError> for ApiError {
    fn from(auth_err: AuthError) -> Self {
        match auth_err {
            AuthError::EmailAlreadyExists => Self {
                status: StatusCode::CONFLICT,
                code: ErrorCode::EmailAlreadyExists,
                message: auth_err.to_string(),
                details: vec![],
            },
            AuthError::EmailAlreadyVerified => Self {
                status: StatusCode::CONFLICT,
                code: ErrorCode::EmailAlreadyExists,
                message: "email already verified".to_owned(),
                details: vec![],
            },
            AuthError::InvalidVerificationToken => Self {
                status: StatusCode::UNAUTHORIZED,
                code: ErrorCode::InvalidCredentials,
                message: "invalid verification token".to_owned(),
                details: vec![],
            },
            AuthError::InvalidCredentials | AuthError::Unauthorized => Self {
                status: StatusCode::UNAUTHORIZED,
                code: ErrorCode::InvalidCredentials,
                message: "invalid credentials".to_owned(),
                details: vec![],
            },
            AuthError::Forbidden => Self {
                status: StatusCode::FORBIDDEN,
                code: ErrorCode::Forbidden,
                message: "forbidden".to_owned(),
                details: vec![],
            },
            AuthError::NotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: ErrorCode::NotFound,
                message: "resource not found".to_owned(),
                details: vec![],
            },
            AuthError::Repository(_) | AuthError::Security(_) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: ErrorCode::InfrastructureUnavailable,
                message: "internal server error".to_owned(),
                details: vec![],
            },
        }
    }
}

impl From<ContributionError> for ApiError {
    fn from(error: ContributionError) -> Self {
        match error {
            ContributionError::Validation(message) => Self {
                status: StatusCode::BAD_REQUEST,
                code: ErrorCode::Validation,
                message,
                details: vec![],
            },
            ContributionError::Forbidden => Self {
                status: StatusCode::FORBIDDEN,
                code: ErrorCode::Forbidden,
                message: "forbidden".to_owned(),
                details: vec![],
            },
            ContributionError::NotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: ErrorCode::NotFound,
                message: "resource not found".to_owned(),
                details: vec![],
            },
            ContributionError::Conflict(message) => Self {
                status: StatusCode::CONFLICT,
                code: ErrorCode::Conflict,
                message,
                details: vec![],
            },
            ContributionError::Repository(_) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: ErrorCode::InfrastructureUnavailable,
                message: "internal server error".to_owned(),
                details: vec![],
            },
        }
    }
}

impl From<CredentialError> for ApiError {
    fn from(error: CredentialError) -> Self {
        match error {
            CredentialError::Validation(message) => Self {
                status: StatusCode::BAD_REQUEST,
                code: ErrorCode::Validation,
                message,
                details: vec![],
            },
            CredentialError::Forbidden => Self {
                status: StatusCode::FORBIDDEN,
                code: ErrorCode::Forbidden,
                message: "forbidden".to_owned(),
                details: vec![],
            },
            CredentialError::NotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: ErrorCode::NotFound,
                message: "resource not found".to_owned(),
                details: vec![],
            },
            CredentialError::Conflict(message) => Self {
                status: StatusCode::CONFLICT,
                code: ErrorCode::Conflict,
                message,
                details: vec![],
            },
            CredentialError::Repository(_) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: ErrorCode::InfrastructureUnavailable,
                message: "internal server error".to_owned(),
                details: vec![],
            },
        }
    }
}

impl From<SchoolError> for ApiError {
    fn from(error: SchoolError) -> Self {
        match error {
            SchoolError::Validation(message) => Self {
                status: StatusCode::BAD_REQUEST,
                code: ErrorCode::Validation,
                message,
                details: vec![],
            },
            SchoolError::Forbidden => Self {
                status: StatusCode::FORBIDDEN,
                code: ErrorCode::Forbidden,
                message: "forbidden".to_owned(),
                details: vec![],
            },
            SchoolError::NotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: ErrorCode::NotFound,
                message: "resource not found".to_owned(),
                details: vec![],
            },
            SchoolError::Conflict(message) => Self {
                status: StatusCode::CONFLICT,
                code: ErrorCode::Conflict,
                message,
                details: vec![],
            },
            SchoolError::Repository(_) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: ErrorCode::InfrastructureUnavailable,
                message: "internal server error".to_owned(),
                details: vec![],
            },
        }
    }
}

impl From<PayoutError> for ApiError {
    fn from(error: PayoutError) -> Self {
        match error {
            PayoutError::Validation(message) => Self {
                status: StatusCode::BAD_REQUEST,
                code: ErrorCode::Validation,
                message,
                details: vec![],
            },
            PayoutError::Forbidden => Self {
                status: StatusCode::FORBIDDEN,
                code: ErrorCode::Forbidden,
                message: "forbidden".to_owned(),
                details: vec![],
            },
            PayoutError::NotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: ErrorCode::NotFound,
                message: "resource not found".to_owned(),
                details: vec![],
            },
            PayoutError::Conflict(message) => Self {
                status: StatusCode::CONFLICT,
                code: ErrorCode::Conflict,
                message,
                details: vec![],
            },
            PayoutError::Repository(_) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: ErrorCode::InfrastructureUnavailable,
                message: "internal server error".to_owned(),
                details: vec![],
            },
        }
    }
}

impl From<NotificationError> for ApiError {
    fn from(error: NotificationError) -> Self {
        match error {
            NotificationError::Validation(message) => Self {
                status: StatusCode::BAD_REQUEST,
                code: ErrorCode::Validation,
                message,
                details: vec![],
            },
            NotificationError::Forbidden => Self {
                status: StatusCode::FORBIDDEN,
                code: ErrorCode::Forbidden,
                message: "forbidden".to_owned(),
                details: vec![],
            },
            NotificationError::NotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: ErrorCode::NotFound,
                message: "resource not found".to_owned(),
                details: vec![],
            },
            NotificationError::Repository(_) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: ErrorCode::InfrastructureUnavailable,
                message: "internal server error".to_owned(),
                details: vec![],
            },
        }
    }
}

impl From<ScholarshipError> for ApiError {
    fn from(error: ScholarshipError) -> Self {
        match error {
            ScholarshipError::Validation(message) => Self {
                status: StatusCode::BAD_REQUEST,
                code: ErrorCode::Validation,
                message,
                details: vec![],
            },
            ScholarshipError::Forbidden => Self {
                status: StatusCode::FORBIDDEN,
                code: ErrorCode::Forbidden,
                message: "forbidden".to_owned(),
                details: vec![],
            },
            ScholarshipError::NotFound => Self {
                status: StatusCode::NOT_FOUND,
                code: ErrorCode::NotFound,
                message: "resource not found".to_owned(),
                details: vec![],
            },
            ScholarshipError::Conflict(message) => Self {
                status: StatusCode::CONFLICT,
                code: ErrorCode::Conflict,
                message,
                details: vec![],
            },
            ScholarshipError::Repository(_) => Self {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                code: ErrorCode::InfrastructureUnavailable,
                message: "internal server error".to_owned(),
                details: vec![],
            },
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(value: sqlx::Error) -> Self {
        Self::from(InternalError::Database(value))
    }
}

impl From<sqlx::migrate::MigrateError> for ApiError {
    fn from(value: sqlx::migrate::MigrateError) -> Self {
        Self::from(InternalError::Migration(value))
    }
}

impl From<InfrastructureError> for ApiError {
    fn from(value: InfrastructureError) -> Self {
        Self::from(InternalError::Infrastructure(value))
    }
}

impl From<std::io::Error> for ApiError {
    fn from(value: std::io::Error) -> Self {
        Self::from(InternalError::Io(value))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorEnvelope {
                error: ErrorBody {
                    code: self.code.as_str(),
                    message: self.message,
                    details: self.details,
                    request_id: None,
                },
            }),
        )
            .into_response()
    }
}
