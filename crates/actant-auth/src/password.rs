//! Argon2id password hashing.
//!
//! Thin wrapper around the [`argon2`] crate so the rest of the codebase
//! never reaches into low-level APIs. We use the crate's `default` Argon2id
//! params; the resulting PHC string travels with the hash so parameter
//! upgrades are automatic on next login.

use actant_core::ActantError;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

/// Minimum password length we accept. Per UI design, very short passwords
/// are rejected at the API boundary to avoid trivial-brute-force exposure.
pub const MIN_PASSWORD_LEN: usize = 8;

/// Hash a plaintext password with Argon2id + a fresh salt. Returns a PHC
/// string ready to store in `workspace_owner.password_hash`.
pub fn hash_password(plaintext: &str) -> Result<String, ActantError> {
    if plaintext.len() < MIN_PASSWORD_LEN {
        return Err(ActantError::InvalidInput(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    argon
        .hash_password(plaintext.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| ActantError::Internal(format!("argon2 hash: {e}")))
}

/// Verify a plaintext against a stored PHC hash. Constant-time via the
/// `argon2` crate's internal compare. Returns `Ok(false)` on a clean
/// mismatch and `Err` only on a malformed hash.
pub fn verify_password(plaintext: &str, phc: &str) -> Result<bool, ActantError> {
    let parsed = PasswordHash::new(phc)
        .map_err(|e| ActantError::Internal(format!("argon2 parse stored hash: {e}")))?;
    match Argon2::default().verify_password(plaintext.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(ActantError::Internal(format!("argon2 verify: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_password() {
        let phc = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &phc).unwrap());
        assert!(!verify_password("wrong password here", &phc).unwrap());
    }

    #[test]
    fn rejects_too_short() {
        assert!(hash_password("short").is_err());
    }

    #[test]
    fn different_salts_yield_different_hashes() {
        let a = hash_password("same-password-please").unwrap();
        let b = hash_password("same-password-please").unwrap();
        assert_ne!(a, b, "fresh salt should produce a different PHC string");
    }
}
