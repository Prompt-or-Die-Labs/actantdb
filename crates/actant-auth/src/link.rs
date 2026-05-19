//! Linking-code primitives.
//!
//! A linking code is the one-time secret printed at boot when an
//! `actantdb-server` instance is bound non-loopback and has no
//! [`workspace_owner`](super) row yet.
//!
//! Format: `xxxx-xxxx-xxxx`, 12 chars of a Crockford-style base32 alphabet
//! (`abcdefghjkmnpqrstuvwxyz23456789`, no `0olO1iIL`), case-insensitive.
//! 12 base32 chars = 60 bits of entropy.
//!
//! The plaintext lives only on the operator's terminal + the user's browser
//! address bar. Storage holds `sha256(lowercased, dashes stripped)`.

use actant_core::ActantError;
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Crockford-style lowercase alphabet, with the confusable pairs
/// `0/o/O`, `1/i/I/l/L` excluded so a misread can't redeem a different
/// code. 31 characters — we draw 5 random bits per character and reject-
/// sample the single out-of-range index. Effective entropy per char is
/// `log2(31) ≈ 4.95`, so 12 chars ≈ 59.5 bits (rounded down to a 60-bit
/// search space in the public banner).
const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";

/// Number of characters in the displayed code (excluding the two dashes).
pub const CODE_LEN: usize = 12;

/// Default time-to-live for a freshly minted linking code.
pub const DEFAULT_TTL_SECS: i64 = 15 * 60;

/// One linking code, in two forms.
#[derive(Debug, Clone)]
pub struct LinkCode {
    /// User-visible code, dashed (`xxxx-xxxx-xxxx`).
    pub display: String,
    /// `sha256(lowercased_no_dashes)`, hex-encoded.
    pub hash: String,
}

impl LinkCode {
    /// Mint a fresh random link code. Uses `OsRng` for entropy.
    pub fn generate() -> Self {
        let mut rng = rand::rngs::OsRng;
        let mut chars = String::with_capacity(CODE_LEN);
        while chars.len() < CODE_LEN {
            // Draw 5 bits per char from a 31-char alphabet; reject the single
            // out-of-range value (index 31) so the distribution stays uniform.
            let mut byte = [0u8; 1];
            rng.fill_bytes(&mut byte);
            let idx = (byte[0] & 0b0001_1111) as usize;
            if idx >= ALPHABET.len() {
                continue;
            }
            chars.push(ALPHABET[idx] as char);
        }
        let display = format!("{}-{}-{}", &chars[0..4], &chars[4..8], &chars[8..12]);
        let hash = hash_code(&display);
        Self { display, hash }
    }
}

/// Normalize an incoming code: lowercase, strip dashes/spaces.
pub fn normalize_code(presented: &str) -> String {
    presented
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Compute the storage hash for a code (after normalization).
pub fn hash_code(presented: &str) -> String {
    let normalized = normalize_code(presented);
    let mut h = Sha256::new();
    h.update(normalized.as_bytes());
    hex::encode(h.finalize())
}

/// Constant-time compare of a presented code against a stored hash. Returns
/// `true` on match. Always hashes the presented value first, then constant-
/// time-compares the resulting hex strings.
pub fn verify_link_code(stored_hash: &str, presented: &str) -> bool {
    let candidate = hash_code(presented);
    if candidate.len() != stored_hash.len() {
        return false;
    }
    candidate
        .as_bytes()
        .ct_eq(stored_hash.as_bytes())
        .unwrap_u8()
        == 1
}

/// Did this character come from the link-code alphabet?
pub fn is_allowed_char(c: char) -> bool {
    let lc = c.to_ascii_lowercase() as u8;
    ALPHABET.contains(&lc)
}

/// Disallowed characters: `0`, `o`, `O`, `1`, `i`, `I`, `l`, `L`.
pub const DISALLOWED_CHARS: &[char] = &['0', 'o', 'O', '1', 'i', 'I', 'l', 'L'];

/// Result of generating a code in shape `(display, hash)`. Convenience for
/// callers that prefer a tuple.
#[must_use]
pub fn generate() -> (String, String) {
    let c = LinkCode::generate();
    (c.display, c.hash)
}

/// Approximate number of bits of entropy in a freshly generated code.
pub const ENTROPY_BITS: usize = CODE_LEN * 5;

/// Validate the alphabet of an incoming user-typed code without consulting
/// the database. Returns `Err` with a user-facing message if the code looks
/// malformed.
pub fn validate_shape(presented: &str) -> Result<(), ActantError> {
    let norm = normalize_code(presented);
    if norm.len() != CODE_LEN {
        return Err(ActantError::InvalidInput(format!(
            "linking code must be {CODE_LEN} characters (got {})",
            norm.len()
        )));
    }
    for c in norm.chars() {
        if !is_allowed_char(c) {
            return Err(ActantError::InvalidInput(format!(
                "linking code contains disallowed character '{c}'"
            )));
        }
    }
    Ok(())
}
