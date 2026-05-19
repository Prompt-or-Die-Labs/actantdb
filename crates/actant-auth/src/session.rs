//! Session token primitives.
//!
//! A session is one opaque random token (256 bits, base64url) that the
//! server hands the browser in a `Set-Cookie: actantdb_session=...`
//! header. Storage holds `sha256(token)` so a leaked DB row never exposes
//! the cookie value.
//!
//! Every session carries its own CSRF secret; mutating routes that
//! authenticate via cookie must echo it back via `X-CSRF-Token`.

use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Session cookie name.
pub const COOKIE_NAME: &str = "actantdb_session";

/// CSRF header name.
pub const CSRF_HEADER: &str = "X-CSRF-Token";

/// Default lifetime of a session token (seconds).
pub const DEFAULT_TTL_SECS: i64 = 30 * 24 * 60 * 60;

/// One newly-minted session.
#[derive(Debug, Clone)]
pub struct SessionToken {
    /// Opaque value placed in the `actantdb_session` cookie.
    pub plaintext: String,
    /// `sha256(plaintext)`, hex-encoded — what we persist.
    pub token_hash: String,
    /// CSRF secret. Returned to the browser in the login/link response body
    /// and matched against `X-CSRF-Token` on mutating routes.
    pub csrf_secret: String,
}

impl SessionToken {
    /// Mint a fresh random session token + CSRF secret.
    pub fn generate() -> Self {
        let plaintext = random_b64(32);
        let csrf_secret = random_b64(32);
        let token_hash = hash_token(&plaintext);
        Self {
            plaintext,
            token_hash,
            csrf_secret,
        }
    }
}

/// Compute the storage hash for a presented session token.
pub fn hash_token(plaintext: &str) -> String {
    let mut h = Sha256::new();
    h.update(plaintext.as_bytes());
    hex::encode(h.finalize())
}

/// Constant-time compare for the CSRF header value vs the stored secret.
pub fn verify_csrf(stored_secret: &str, presented: &str) -> bool {
    if stored_secret.len() != presented.len() {
        return false;
    }
    stored_secret
        .as_bytes()
        .ct_eq(presented.as_bytes())
        .unwrap_u8()
        == 1
}

/// Generate `n` bytes of OS randomness, base64url-encode (no padding).
fn random_b64(n: usize) -> String {
    let mut buf = vec![0u8; n];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&buf)
}
