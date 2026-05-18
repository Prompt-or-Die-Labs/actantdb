//! Positive + negative coverage for every operator in the Phase 4 v1 DSL.
//!
//! Operators (5): `must_emit`, `must_not_emit`, `cost_le`, `latency_le_ms`,
//! `assert.jsonpath`. Each has one positive and one negative test, plus a
//! handful of edge-case tests that exercise resolution and graceful failure.

use actant_eval::{AssertOp, Criterion, Event, SuccessCriteria};
use serde_json::json;

fn ev(t: &str) -> Event {
    Event {
        event_type: t.into(),
        cost: None,
        latency_ms: None,
        payload: json!({}),
    }
}

fn ev_full(
    t: &str,
    cost: Option<f64>,
    latency_ms: Option<u64>,
    payload: serde_json::Value,
) -> Event {
    Event {
        event_type: t.into(),
        cost,
        latency_ms,
        payload,
    }
}

#[test]
fn must_emit_positive() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::MustEmit("tool_call_finished".into())],
    };
    let events = vec![ev("tool_call_started"), ev("tool_call_finished")];
    let r = crit.evaluate(&events);
    assert!(r.passed, "expected pass, got failures: {:?}", r.failures);
}

#[test]
fn must_emit_negative() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::MustEmit("tool_call_finished".into())],
    };
    let events = vec![ev("tool_call_started")];
    let r = crit.evaluate(&events);
    assert!(!r.passed);
    assert_eq!(r.failures.len(), 1);
    assert!(r.failures[0].contains("must_emit"));
    assert!(r.failures[0].contains("tool_call_finished"));
}

#[test]
fn must_not_emit_positive() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::MustNotEmit("policy_blocked".into())],
    };
    let events = vec![ev("tool_call_finished")];
    let r = crit.evaluate(&events);
    assert!(r.passed, "expected pass, got failures: {:?}", r.failures);
}

#[test]
fn must_not_emit_negative() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::MustNotEmit("policy_blocked".into())],
    };
    let events = vec![ev("tool_call_started"), ev("policy_blocked")];
    let r = crit.evaluate(&events);
    assert!(!r.passed);
    assert!(r.failures[0].contains("must_not_emit"));
    assert!(r.failures[0].contains("policy_blocked"));
}

#[test]
fn cost_le_positive() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::CostLe(0.05)],
    };
    let events = vec![
        ev_full("model_call_finished", Some(0.01), None, json!({})),
        ev_full("model_call_finished", Some(0.02), None, json!({})),
    ];
    let r = crit.evaluate(&events);
    assert!(
        r.passed,
        "expected pass at 0.03 <= 0.05, got: {:?}",
        r.failures
    );
}

#[test]
fn cost_le_negative() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::CostLe(0.05)],
    };
    let events = vec![
        ev_full("model_call_finished", Some(0.04), None, json!({})),
        ev_full("model_call_finished", Some(0.03), None, json!({})),
    ];
    let r = crit.evaluate(&events);
    assert!(!r.passed);
    assert!(r.failures[0].contains("cost_le"));
}

#[test]
fn latency_le_ms_positive() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::LatencyLeMs(5000)],
    };
    let events = vec![
        ev_full("tool_call_finished", None, Some(1200), json!({})),
        ev_full("tool_call_finished", None, Some(4800), json!({})),
    ];
    let r = crit.evaluate(&events);
    assert!(
        r.passed,
        "expected pass with max=4800 <= 5000, got: {:?}",
        r.failures
    );
}

#[test]
fn latency_le_ms_negative() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::LatencyLeMs(5000)],
    };
    let events = vec![
        ev_full("tool_call_finished", None, Some(100), json!({})),
        ev_full("tool_call_finished", None, Some(6500), json!({})),
    ];
    let r = crit.evaluate(&events);
    assert!(!r.passed);
    assert!(r.failures[0].contains("latency_le_ms"));
    assert!(r.failures[0].contains("6500"));
}

#[test]
fn assert_jsonpath_positive() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::Assert {
            jsonpath: "$.output_tokens".into(),
            op: AssertOp::Lt,
            value: json!(800),
        }],
    };
    let events = vec![ev_full(
        "model_call_finished",
        None,
        None,
        json!({"output_tokens": 412}),
    )];
    let r = crit.evaluate(&events);
    assert!(r.passed, "412 < 800 should pass, got: {:?}", r.failures);
}

#[test]
fn assert_jsonpath_negative() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::Assert {
            jsonpath: "$.output_tokens".into(),
            op: AssertOp::Lt,
            value: json!(800),
        }],
    };
    let events = vec![ev_full(
        "model_call_finished",
        None,
        None,
        json!({"output_tokens": 1024}),
    )];
    let r = crit.evaluate(&events);
    assert!(!r.passed);
    assert!(r.failures[0].contains("assert"));
}

#[test]
fn assert_missing_path_is_failure_not_panic() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::Assert {
            jsonpath: "$.does.not.exist".into(),
            op: AssertOp::Eq,
            value: json!(1),
        }],
    };
    let events = vec![ev_full("anything", None, None, json!({"a": 1}))];
    let r = crit.evaluate(&events);
    assert!(!r.passed, "missing path should fail, not pass");
    assert!(
        r.failures[0].contains("did not resolve"),
        "got: {:?}",
        r.failures
    );
}

#[test]
fn assert_array_index_path() {
    let crit = SuccessCriteria {
        all_of: vec![Criterion::Assert {
            jsonpath: "$.events[0].name".into(),
            op: AssertOp::Eq,
            value: json!("first"),
        }],
    };
    let events = vec![ev_full(
        "session_finished",
        None,
        None,
        json!({"events": [{"name": "first"}, {"name": "second"}]}),
    )];
    let r = crit.evaluate(&events);
    assert!(r.passed, "expected pass, got: {:?}", r.failures);
}

#[test]
fn multiple_criteria_aggregate_failures() {
    let crit = SuccessCriteria {
        all_of: vec![
            Criterion::MustEmit("tool_call_finished".into()),
            Criterion::CostLe(0.01),
            Criterion::LatencyLeMs(100),
        ],
    };
    let events = vec![ev_full(
        "model_call_finished",
        Some(1.0),
        Some(500),
        json!({}),
    )];
    let r = crit.evaluate(&events);
    assert!(!r.passed);
    assert_eq!(r.failures.len(), 3, "all three criteria should fail");
}

#[test]
fn json_round_trip_for_versioned_storage() {
    // SuccessCriteria is stored as a versioned artifact (see /agents/actant-eval.md).
    // Ensure the snake_case wire form parses.
    let src = r#"{
        "all_of": [
            { "must_emit":     "tool_call_finished" },
            { "must_not_emit": "policy_blocked" },
            { "cost_le":       0.05 },
            { "latency_le_ms": 5000 },
            { "assert": { "jsonpath": "$.output_tokens", "op": "lt", "value": 800 } }
        ]
    }"#;
    let crit: SuccessCriteria = serde_json::from_str(src).expect("parse SuccessCriteria");
    assert_eq!(crit.all_of.len(), 5);
}
