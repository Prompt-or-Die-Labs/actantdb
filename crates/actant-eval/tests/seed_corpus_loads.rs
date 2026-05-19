//! Lock the `evals/seed/` corpus: every JSON file parses into a complete
//! EvalCase + at least one Criterion. Catches schema drift.

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize)]
struct SeedCase {
    id: String,
    name: String,
    expected_behavior: String,
    forbidden_behavior: Option<String>,
    success_criteria: serde_json::Value,
}

#[test]
fn every_seed_case_parses() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("evals")
        .join("seed");
    assert!(dir.is_dir(), "evals/seed dir missing at {}", dir.display());

    let mut count = 0;
    for entry in fs::read_dir(&dir).expect("read evals/seed") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let body = fs::read_to_string(&path).expect("read");
        let case: SeedCase = serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("seed case {} failed to parse: {e}", path.display()));
        assert!(!case.id.is_empty());
        assert!(!case.name.is_empty());
        assert!(!case.expected_behavior.is_empty());
        // success_criteria.all_of must exist and be a non-empty array.
        let all_of = case
            .success_criteria
            .get("all_of")
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| {
                panic!(
                    "seed case {} missing success_criteria.all_of array",
                    path.display()
                )
            });
        assert!(
            !all_of.is_empty(),
            "seed case {} has empty all_of",
            path.display()
        );
        count += 1;
    }
    assert!(count >= 8, "expected at least 8 seed cases, got {count}");
}
