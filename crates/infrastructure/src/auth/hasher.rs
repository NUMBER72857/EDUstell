use application::auth::AuthError;
use application::ports::PasswordHasher;
use argon2::{
    Argon2,
    password_hash::{
        PasswordHash, PasswordHasher as _, PasswordVerifier, SaltString, rand_core::OsRng,
    },
};
use async_trait::async_trait;

#[derive(Debug, Default, Clone)]
pub struct Argon2PasswordHasher;

#[async_trait]
impl PasswordHasher for Argon2PasswordHasher {
    async fn hash_secret(&self, value: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(value.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|err| AuthError::Security(format!("failed to hash secret: {err}")))
    }

    async fn verify_secret(&self, value: &str, hash: &str) -> Result<bool, AuthError> {
        let parsed = PasswordHash::new(hash)
            .map_err(|err| AuthError::Security(format!("invalid password hash: {err}")))?;

        Ok(Argon2::default().verify_password(value.as_bytes(), &parsed).is_ok())
    }
}
