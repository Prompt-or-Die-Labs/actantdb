//! Policy bundle that travels with derived content.
//!
//! Capsule contract types are defined in `actant-contracts`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use actant_contracts::{ActantCapsule, MemoryAllowed};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_deny_blocks_cloud() {
        assert!(!ActantCapsule::default_deny("x").cloud_allowed());
        assert!(ActantCapsule::public_okay("y").cloud_allowed());
    }

    #[test]
    fn memory_allowed_has_fixed_json_vocabulary() {
        let capsule = ActantCapsule::public_okay("public");
        let encoded = serde_json::to_string(&capsule).unwrap();
        assert!(encoded.contains("\"memory_allowed\":\"workspace\""));

        let decoded: ActantCapsule = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.memory_allowed, MemoryAllowed::Workspace);
    }
}
