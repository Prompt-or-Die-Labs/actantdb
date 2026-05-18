//! actant-eval — eval case + run lifecycle.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod dsl;

pub use dsl::{AssertOp, Criterion, EvalResult, Event, SuccessCriteria};

use serde::{Deserialize, Serialize};

/// Eval case — replay-derived test that runs forever after.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    /// Identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Expected behavior text.
    pub expected_behavior: String,
    /// Forbidden behavior text (optional).
    pub forbidden_behavior: Option<String>,
    /// Programmatic success criteria (free-form).
    pub success_criteria: String,
}

/// Run an eval. Returns true if the actual behavior matches the expected.
pub fn run(case: &EvalCase, actual: &str) -> bool {
    let matches_expected = actual.contains(&case.expected_behavior);
    let avoids_forbidden = case
        .forbidden_behavior
        .as_ref()
        .map(|f| !actual.contains(f))
        .unwrap_or(true);
    matches_expected && avoids_forbidden
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_expected() {
        let c = EvalCase {
            id: "e1".into(),
            name: "rm dist should be constrained".into(),
            expected_behavior: "constrained variant accepted".into(),
            forbidden_behavior: Some("dist deleted".into()),
            success_criteria: "constrained variant accepted".into(),
        };
        assert!(run(&c, "result: constrained variant accepted"));
        assert!(!run(
            &c,
            "result: constrained variant accepted but dist deleted"
        ));
        assert!(!run(&c, "result: dist deleted"));
    }
}
