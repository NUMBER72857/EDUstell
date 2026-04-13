use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRole {
    Parent,
    Contributor,
    Student,
    SchoolAdmin,
    Donor,
    PlatformAdmin,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Parent => "parent",
            Self::Contributor => "contributor",
            Self::Student => "student",
            Self::SchoolAdmin => "school_admin",
            Self::Donor => "donor",
            Self::PlatformAdmin => "platform_admin",
        }
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for UserRole {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "parent" => Ok(Self::Parent),
            "contributor" => Ok(Self::Contributor),
            "student" => Ok(Self::Student),
            "school_admin" => Ok(Self::SchoolAdmin),
            "donor" => Ok(Self::Donor),
            "platform_admin" => Ok(Self::PlatformAdmin),
            _ => Err("invalid user role"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub role: UserRole,
    pub email_verified: bool,
    pub mfa_enabled: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub email: String,
    pub password_hash: String,
    pub role: UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessClaims {
    pub sub: Uuid,
    pub role: UserRole,
    pub session_id: Uuid,
    pub jti: Uuid,
    pub exp: i64,
    pub iat: i64,
    pub token_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshClaims {
    pub sub: Uuid,
    pub session_id: Uuid,
    pub exp: i64,
    pub iat: i64,
    pub token_type: String,
}

#[derive(Debug, Clone)]
pub struct RefreshSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct EmailVerificationToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicUser {
    pub id: Uuid,
    pub email: String,
    pub role: UserRole,
    pub email_verified: bool,
    pub mfa_enabled: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<User> for PublicUser {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            email: value.email,
            role: value.role,
            email_verified: value.email_verified,
            mfa_enabled: value.mfa_enabled,
            status: value.status,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub role: UserRole,
    pub session_id: Uuid,
}
