//! Spec 18 — Reliability primitives verification.

use std::fs;
use std::path::Path;

fn read_repo(path: &str) -> String {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let p = here.parent().unwrap().parent().unwrap().join(path);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn every_reliability_table_in_migration_0003() {
    let mig = read_repo("migrations/0003_ai_native_and_reliability.sql");
    for table in [
        "rate_limit_policy",
        "rate_limit_state",
        "retry_policy",
        "circuit_state",
        "dead_letter_item",
        "lock",
        "ingress_event",
        "idempotency_record",
        "effect_queue_entry",
    ] {
        assert!(
            mig.contains(&format!("CREATE TABLE {table}")),
            "migration 0003 missing reliability table {table}"
        );
    }
}

#[test]
fn token_bucket_invariant_holds() {
    use actant_throttle::{Bucket, Policy};
    let mut b = Bucket::new(Policy {
        limit: 5,
        refill_per_second: 1.0,
    });
    // Drain.
    for _ in 0..5 {
        assert!(b.try_consume(1).is_ok());
    }
    // Next consume must fail.
    assert!(b.try_consume(1).is_err());
}

#[test]
fn circuit_state_invariant_holds() {
    use actant_circuit::{Breaker, Policy, State};
    use std::time::Duration;
    let mut b = Breaker::new(Policy {
        failure_threshold: 1,
        open_duration: Duration::from_millis(10),
    });
    assert_eq!(b.current(), State::Closed);
    b.on_failure();
    assert_eq!(b.current(), State::Open);
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(b.current(), State::HalfOpen);
}
