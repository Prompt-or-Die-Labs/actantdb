//! Property checks on link-code generation.
//!
//! Pins the contract the boot banner depends on:
//!   * 12 chars of base32 alphabet → 60 bits of entropy.
//!   * Alphabet excludes the confusing pairs `0/o/O`, `1/i/I/l/L`.
//!   * Storage form is `sha256(lowercased, dashes stripped)`, hex-encoded.
//!   * Display form is `xxxx-xxxx-xxxx`, lowercase.

use std::collections::HashSet;

use actant_auth::link::{
    hash_code, normalize_code, validate_shape, CODE_LEN, DISALLOWED_CHARS, ENTROPY_BITS,
};
use actant_auth::{verify_link_code, LinkCode};
use sha2::{Digest, Sha256};

#[test]
fn entropy_is_sixty_bits() {
    assert_eq!(ENTROPY_BITS, 60, "12 chars of base32 = 60 bits");
}

#[test]
fn display_form_is_xxxx_xxxx_xxxx() {
    let c = LinkCode::generate();
    assert_eq!(c.display.len(), CODE_LEN + 2, "two dashes");
    let parts: Vec<&str> = c.display.split('-').collect();
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|p| p.len() == 4));
    // All-lowercase.
    assert_eq!(c.display, c.display.to_lowercase());
}

#[test]
fn alphabet_excludes_confusing_chars() {
    // Stress: generate 256 codes and union their characters; assert nothing
    // from the disallowed set ever appears.
    let mut seen: HashSet<char> = HashSet::new();
    for _ in 0..256 {
        let c = LinkCode::generate();
        for ch in c.display.chars() {
            if ch != '-' {
                seen.insert(ch);
            }
        }
    }
    for bad in DISALLOWED_CHARS {
        assert!(
            !seen.contains(bad),
            "alphabet must exclude confusing char {bad:?}"
        );
    }
}

#[test]
fn stored_hash_is_sha256_of_normalized_form() {
    let c = LinkCode::generate();
    // Recompute expected hash manually so we'd notice if the format ever drifts.
    let normalized = normalize_code(&c.display);
    let mut h = Sha256::new();
    h.update(normalized.as_bytes());
    let expected = hex::encode(h.finalize());
    assert_eq!(c.hash, expected);
    assert_eq!(c.hash.len(), 64, "sha256 hex = 64 chars");
}

#[test]
fn verify_link_code_matches_dashed_or_undashed() {
    let c = LinkCode::generate();
    // User can type the dashed form, the smashed form, or upper-case.
    assert!(verify_link_code(&c.hash, &c.display));
    let smashed: String = c.display.chars().filter(|&c| c != '-').collect();
    assert!(verify_link_code(&c.hash, &smashed));
    assert!(verify_link_code(&c.hash, &smashed.to_uppercase()));
}

#[test]
fn verify_link_code_rejects_wrong_value() {
    let c = LinkCode::generate();
    assert!(!verify_link_code(&c.hash, "abcd-efgh-jkmn"));
    assert!(!verify_link_code(&c.hash, ""));
    assert!(!verify_link_code(&c.hash, "xxxxxxxxxxxxx")); // wrong length
}

#[test]
fn validate_shape_rejects_disallowed_characters() {
    // Pure digits/letters from the allowed alphabet should pass.
    assert!(validate_shape("abcd-efgh-jkmn").is_ok());
    // A '0' or '1' should be flagged as a typo immediately.
    assert!(validate_shape("abcd-0fgh-jkmn").is_err());
    assert!(validate_shape("abcd-efgh-1kmn").is_err());
    assert!(validate_shape("abcd-efgh").is_err()); // too short
}

#[test]
fn generates_distinct_codes() {
    // Birthday paradox: 60-bit space, 1k samples → collision probability is
    // ~ 1k^2 / 2^61 ≈ 4 × 10⁻¹³. A flake here means we broke OsRng.
    let mut seen = HashSet::with_capacity(1024);
    for _ in 0..1024 {
        let c = LinkCode::generate();
        assert!(
            seen.insert(c.display),
            "collision is a randomness regression"
        );
    }
}

#[test]
fn hash_code_is_independent_of_dashes_and_case() {
    let a = hash_code("ABCD-EFGH-JKMN");
    let b = hash_code("abcdefghjkmn");
    let c = hash_code("abcd-efgh-jkmn");
    assert_eq!(a, b);
    assert_eq!(b, c);
}
