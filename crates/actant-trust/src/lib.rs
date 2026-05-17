//! actant-trust — behavior-derived authority calibration.
//!
//! Each actor accumulates a per-capability trust score in [0, 1] backed by
//! sample count. Higher trust unlocks auto-approval at lower risk levels.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// Trust profile for one actor + capability area.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustProfile {
    /// Capability area (e.g. "tool:shell.run", "memory.propose").
    pub area: String,
    /// Score in \[0, 1].
    pub score: f64,
    /// Bayesian confidence given sample size.
    pub confidence: f64,
    /// Number of observations.
    pub sample_size: u64,
}

impl TrustProfile {
    /// Cold start.
    pub fn new(area: impl Into<String>) -> Self {
        Self {
            area: area.into(),
            score: 0.5,
            confidence: 0.0,
            sample_size: 0,
        }
    }

    /// Record a success.
    pub fn observe(&mut self, success: bool) {
        let outcome = if success { 1.0 } else { 0.0 };
        let n = self.sample_size as f64;
        self.score = (self.score * n + outcome) / (n + 1.0);
        self.sample_size += 1;
        // Wilson-ish confidence — saturates as N grows.
        self.confidence = self.sample_size as f64 / (self.sample_size as f64 + 10.0);
    }

    /// Returns true when this profile permits auto-approval at the given
    /// minimum required confidence threshold (heuristic for the alpha demo).
    pub fn auto_approve_at(&self, min_score: f64, min_confidence: f64) -> bool {
        self.score >= min_score && self.confidence >= min_confidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observations_raise_confidence() {
        let mut p = TrustProfile::new("tool:file.read");
        for _ in 0..20 {
            p.observe(true);
        }
        assert!(p.confidence > 0.5);
        assert!(p.auto_approve_at(0.8, 0.5));
    }
}
