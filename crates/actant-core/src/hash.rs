//! Stable hashing helpers used across the substrate.

use sha2::{Digest, Sha256};

/// Canonical JSON serialization — keys sorted, no whitespace. Drives stable
/// content hashing of payloads.
pub fn canonical_json(value: &serde_json::Value) -> String {
    canonicalize(value).to_string()
}

fn canonicalize(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: std::collections::BTreeMap<String, serde_json::Value> =
                std::collections::BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.clone(), canonicalize(v));
            }
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonicalize).collect())
        }
        other => other.clone(),
    }
}

/// SHA-256 of an arbitrary string, lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// Hash of (prev_chain_hash, payload_hash) for tamper-evident chaining.
pub fn chain_hash(prev: &str, payload_hash: &str) -> String {
    sha256_hex(format!("{prev}:{payload_hash}").as_bytes())
}
