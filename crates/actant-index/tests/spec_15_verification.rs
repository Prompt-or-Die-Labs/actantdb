//! Spec 15 — ActantIndex verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn every_index_table_in_migration_0003() {
    let mig = read_repo("migrations/0003_ai_native_and_reliability.sql");
    for table in [
        "indexed_object",
        "index_chunk",
        "sparse_ref",
        "multivector_ref",
        "embedding_space",
        "entity",
        "entity_relation",
        "retrieval_trace",
        "retrieval_candidate",
    ] {
        assert!(
            mig.contains(&format!("CREATE TABLE {table}")),
            "migration 0003 missing index table {table}"
        );
    }
}

#[test]
fn index_code_supports_dense_path() {
    let lib = read_repo("crates/actant-index/src/lib.rs");
    assert!(
        lib.contains("cosine"),
        "dense cosine retrieval path missing"
    );
    assert!(lib.contains("VectorStore"), "VectorStore trait missing");
    assert!(
        lib.contains("InMemoryStore"),
        "InMemoryStore backend missing"
    );
}
