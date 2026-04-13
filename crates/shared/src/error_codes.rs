#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    Internal,
    InfrastructureUnavailable,
    Validation,
    InvalidCredentials,
    Forbidden,
    NotFound,
    Conflict,
    EmailAlreadyExists,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Internal => "INTERNAL_ERROR",
            Self::InfrastructureUnavailable => "INFRASTRUCTURE_UNAVAILABLE",
            Self::Validation => "VALIDATION_ERROR",
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::Forbidden => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::EmailAlreadyExists => "EMAIL_ALREADY_EXISTS",
        }
    }
}
