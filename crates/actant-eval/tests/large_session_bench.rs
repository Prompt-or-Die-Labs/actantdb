//! AC: "Running an eval against a 1000-event session completes in under 30s
//! on a laptop with `mode=recorded`."
//!
//! This is wildly over-budget — the in-memory evaluator is O(N * |criteria|) over
//! events with no I/O. The test exists to lock the AC and catch a future
//! regression that wires in a synchronous network call or quadratic resolver.

use actant_eval::{Criterion, Event, SuccessCriteria};
use serde_json::json;
use std::time::Instant;

#[test]
fn one_thousand_events_under_thirty_seconds() {
    let mut events: Vec<Event> = Vec::with_capacity(1000);
    for i in 0..1000 {
        events.push(Event {
            event_type: if i == 999 {
                "tool_call_finished".into()
            } else {
                "tool_call_progress".into()
            },
            cost: Some(0.00001),
            latency_ms: Some((i % 250) as u64),
            payload: json!({ "i": i, "nested": { "key": format!("v{i}") } }),
        });
    }

    let crit = SuccessCriteria {
        all_of: vec![
            Criterion::MustEmit("tool_call_finished".into()),
            Criterion::CostLe(0.5), // sum is 0.01
            Criterion::LatencyLeMs(250),
        ],
    };

    let start = Instant::now();
    let result = crit.evaluate(&events);
    let elapsed = start.elapsed();

    assert!(
        result.passed,
        "expected pass on synthetic session, got failures: {:?}",
        result.failures
    );
    assert!(
        elapsed.as_secs() < 30,
        "evaluator took {elapsed:?}, AC requires < 30s"
    );
    eprintln!("1000-event evaluate: {elapsed:?}");
}
