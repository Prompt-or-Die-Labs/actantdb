//! Spec 06 — context + memory verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn every_memory_state_appears_in_schema() {
    // Spec 06: "Every memory state appears in 02-data-model.sql"
    let schema = read_repo("specs/02-data-model.sql");
    for col in [
        "memory_candidate",
        "deleted_at",
        "revoked_at",
        "expires_at",
        "memory_candidate",
    ] {
        assert!(
            schema.contains(col),
            "spec 06 references memory state column {col} which is missing from spec 02"
        );
    }
}

#[test]
fn blocked_reason_values_match() {
    // Spec 06: "Every blocked_reason value used in the pipeline appears in §3"
    // Our pipeline currently produces these reasons:
    let lib = read_repo("crates/actant-context/src/lib.rs");
    // Per spec 06 §3 the enumerated values are sensitivity / visibility / budget.
    let produced = ["sensitivity", "visibility", "budget"];
    let spec_06 = read_repo("specs/06-context-and-memory.md");
    for r in produced {
        assert!(
            lib.contains(&format!("\"{r}\"")),
            "actant-context produces unknown blocked_reason {r}"
        );
        assert!(
            spec_06.contains(r),
            "blocked_reason {r} produced but not documented in spec 06"
        );
    }
}
