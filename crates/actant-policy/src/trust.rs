//! Behavior-derived authority calibration.
//!
//! Trust contract types are defined in `actant-contracts`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use actant_contracts::ActantTrustProfile;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observations_raise_confidence() {
        let mut p = ActantTrustProfile::new("tool:file.read");
        for _ in 0..20 {
            p.observe(true);
        }
        assert!(p.confidence > 0.5);
        assert!(p.auto_approve_at(0.8, 0.5));
    }

    #[test]
    fn first_observation_preserves_neutral_prior() {
        let mut success = ActantTrustProfile::new("tool:file.read");
        success.observe(true);
        assert!(success.score > 0.5);
        assert!(success.score < 1.0);

        let mut failure = ActantTrustProfile::new("tool:file.read");
        failure.observe(false);
        assert!(failure.score > 0.0);
        assert!(failure.score < 0.5);
    }
}
