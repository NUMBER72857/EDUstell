use std::{env, net::SocketAddr, str::FromStr};

use crate::error::InternalError;

#[derive(Debug, Clone)]
pub struct Config {
    pub app_name: String,
    pub environment: Environment,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub jwt: JwtConfig,
    pub observability: ObservabilityConfig,
}

impl Config {
    pub fn from_env() -> Result<Self, InternalError> {
        let config = Self {
            app_name: read_var("APP_NAME")?,
            environment: read_var("APP_ENV")?.parse()?,
            server: ServerConfig { host: read_var("APP_HOST")?, port: read_parsed("APP_PORT")? },
            database: DatabaseConfig { url: read_var("DATABASE_URL")? },
            jwt: JwtConfig {
                access_secret: read_var("JWT_ACCESS_SECRET")?,
                refresh_secret: read_var("JWT_REFRESH_SECRET")?,
                access_ttl_secs: read_parsed("JWT_ACCESS_TTL_SECS")?,
                refresh_ttl_secs: read_parsed("JWT_REFRESH_TTL_SECS")?,
            },
            observability: ObservabilityConfig {
                rust_log: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned()),
                log_format: read_var("LOG_FORMAT")?,
            },
        };

        config.validate()?;
        Ok(config)
    }
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl ServerConfig {
    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port).parse().expect("validated server address")
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub access_secret: String,
    pub refresh_secret: String,
    pub access_ttl_secs: i64,
    pub refresh_ttl_secs: i64,
}

#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub rust_log: String,
    pub log_format: String,
}

#[derive(Debug, Clone, Copy)]
pub enum Environment {
    Local,
    Development,
    Staging,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Development => "development",
            Self::Staging => "staging",
            Self::Production => "production",
        }
    }
}

impl FromStr for Environment {
    type Err = InternalError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "local" => Ok(Self::Local),
            "development" => Ok(Self::Development),
            "staging" => Ok(Self::Staging),
            "production" => Ok(Self::Production),
            other => Err(InternalError::Config(format!("invalid APP_ENV value: {other}"))),
        }
    }
}

impl Config {
    fn validate(&self) -> Result<(), InternalError> {
        if self.jwt.access_secret.len() < 32 {
            return Err(InternalError::Config(
                "JWT_ACCESS_SECRET must be at least 32 characters".to_owned(),
            ));
        }
        if self.jwt.refresh_secret.len() < 32 {
            return Err(InternalError::Config(
                "JWT_REFRESH_SECRET must be at least 32 characters".to_owned(),
            ));
        }
        if self.jwt.access_secret == self.jwt.refresh_secret {
            return Err(InternalError::Config(
                "JWT_ACCESS_SECRET and JWT_REFRESH_SECRET must differ".to_owned(),
            ));
        }
        if matches!(self.environment, Environment::Staging | Environment::Production)
            && self.observability.log_format != "json"
        {
            return Err(InternalError::Config(
                "LOG_FORMAT must be json outside local/development".to_owned(),
            ));
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn test() -> Self {
        Self {
            app_name: "EDUstell".to_owned(),
            environment: Environment::Local,
            server: ServerConfig { host: "127.0.0.1".to_owned(), port: 8080 },
            database: DatabaseConfig {
                url: "postgres://postgres:postgres@localhost:5432/edustell".to_owned(),
            },
            jwt: JwtConfig {
                access_secret: "access-secret-access-secret-1234".to_owned(),
                refresh_secret: "refresh-secret-refresh-secret-1234".to_owned(),
                access_ttl_secs: 900,
                refresh_ttl_secs: 2_592_000,
            },
            observability: ObservabilityConfig {
                rust_log: "info".to_owned(),
                log_format: "pretty".to_owned(),
            },
        }
    }
}

fn read_var(key: &'static str) -> Result<String, InternalError> {
    env::var(key).map_err(|_| InternalError::Config(format!("missing required env var {key}")))
}

fn read_parsed<T>(key: &'static str) -> Result<T, InternalError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let value = read_var(key)?;
    value.parse::<T>().map_err(|err| InternalError::Config(format!("invalid {key}: {err}")))
}
