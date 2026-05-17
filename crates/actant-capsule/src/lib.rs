//! actant-capsule — policy bundle that travels with derived content.
//!
//! A capsule binds a (sensitivity, visibility, redaction, retention) policy
//! to a unit of content; downstream commands consult it before exposure.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::Sensitivity;
use serde::{Deserialize, Serialize};

/// Capsule body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capsule {
    /// Display name.
    pub name: String,
    /// Highest sensitivity content this capsule may hold.
    pub sensitivity: Sensitivity,
    /// Allow this capsule's content to be sent to cloud models.
    pub cloud_model_allowed: bool,
    /// Memory storage allowed (`forbidden|workspace|global`).
    pub memory_allowed: String,
    /// Sensitivity to upgrade to when re-emitted as memory.
    pub upgrades_to_sensitivity: Option<Sensitivity>,
}

impl Capsule {
    /// Default-deny capsule for unknown content.
    pub fn default_deny(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sensitivity: Sensitivity::High,
            cloud_model_allowed: false,
            memory_allowed: "forbidden".into(),
            upgrades_to_sensitivity: None,
        }
    }

    /// Visible cloud-allowed capsule (for the alpha demo).
    pub fn public_okay(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sensitivity: Sensitivity::Low,
            cloud_model_allowed: true,
            memory_allowed: "workspace".into(),
            upgrades_to_sensitivity: None,
        }
    }

    /// Returns true if this capsule permits routing to a cloud model.
    pub fn cloud_allowed(&self) -> bool {
        self.cloud_model_allowed && !matches!(self.sensitivity, Sensitivity::Secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_deny_blocks_cloud() {
        assert!(!Capsule::default_deny("x").cloud_allowed());
        assert!(Capsule::public_okay("y").cloud_allowed());
    }
}
