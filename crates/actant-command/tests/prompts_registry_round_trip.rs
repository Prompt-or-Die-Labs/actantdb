//! Spec — prompt registry round-trip.
//!
//! The work-package's first "Tests" bullet is
//! "Round-trip: create + add_version + retrieve by `(name, version)`". The
//! v0.1 in-memory `ActantPromptTemplate` exposes that round-trip directly: construct a
//! template, push one or more `ActantPromptVersion`s, and retrieve by `(name, version)`
//! via `ActantPromptTemplate::render`. This file pins that behaviour so future moves to
//! a Storage-backed `PromptService` (per the spec) preserve the contract.

use actant_command::prompts::{ActantPromptTemplate, ActantPromptVersion};

#[test]
fn create_add_version_retrieve_by_name_and_version() {
    let mut t = ActantPromptTemplate {
        name: "code_review".into(),
        versions: vec![],
    };
    t.versions.push(ActantPromptVersion {
        version: 1,
        body: "Review this code: {{snippet}}".into(),
    });
    t.versions.push(ActantPromptVersion {
        version: 2,
        body: "Carefully review the following: {{snippet}}".into(),
    });

    assert_eq!(t.name, "code_review");
    assert_eq!(t.versions.len(), 2);

    let r1 = t
        .render(1, &serde_json::json!({"snippet": "let x = 1;"}))
        .expect("v1 renders");
    assert_eq!(r1, "Review this code: let x = 1;");

    let r2 = t
        .render(2, &serde_json::json!({"snippet": "let x = 1;"}))
        .expect("v2 renders");
    assert_eq!(r2, "Carefully review the following: let x = 1;");

    assert_eq!(t.latest().unwrap().version, 2);
}

#[test]
fn missing_version_returns_none() {
    let t = ActantPromptTemplate {
        name: "empty".into(),
        versions: vec![ActantPromptVersion {
            version: 1,
            body: "hi".into(),
        }],
    };
    assert!(t.render(99, &serde_json::json!({})).is_none());
}

#[test]
fn unknown_variables_render_empty() {
    let t = ActantPromptTemplate {
        name: "vars".into(),
        versions: vec![ActantPromptVersion {
            version: 1,
            body: "x={{a}} y={{b}}".into(),
        }],
    };
    let out = t.render(1, &serde_json::json!({"a": "1"})).unwrap();
    assert_eq!(out, "x=1 y=");
}
