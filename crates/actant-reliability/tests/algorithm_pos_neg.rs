//! Positive and negative cases for every rate-limit algorithm implemented
//! in this crate.
//!
//! Spec/AC adjustment: the work package enumerates token-bucket,
//! leaky-bucket, fixed-window, sliding-window, concurrency, weighted-fair-
//! queue, priority, deadline, and adaptive. The current
//! `crates/actant-reliability/src/throttle.rs` ships only the token-bucket
//! primitive (`Bucket` + `Policy`). We test pos+neg for that algorithm and
//! flag the rest as TODO via ignored placeholder tests so the gap is
//! visible in `cargo test` output.

use actant_reliability::throttle::{Bucket, Policy};
use std::time::Duration;

#[test]
fn token_bucket_positive_request_within_limit_allowed() {
    let mut b = Bucket::new(Policy {
        limit: 5,
        refill_per_second: 1.0,
    });
    assert!(
        b.try_consume(3).is_ok(),
        "consuming 3 of 5 tokens should succeed on a full bucket"
    );
}

#[test]
fn token_bucket_negative_request_over_limit_rejected() {
    let mut b = Bucket::new(Policy {
        limit: 5,
        refill_per_second: 1.0,
    });
    // Drain.
    for _ in 0..5 {
        assert!(b.try_consume(1).is_ok());
    }
    // Sixth consume must reject and return a non-zero retry-after.
    let err = b
        .try_consume(1)
        .expect_err("over-limit consume must be Err");
    assert!(err > Duration::ZERO);
}

#[test]
#[ignore = "TODO: leaky-bucket algorithm not implemented in actant-reliability::throttle"]
fn leaky_bucket_pos_neg_placeholder() {}

#[test]
#[ignore = "TODO: fixed-window algorithm not implemented in actant-reliability::throttle"]
fn fixed_window_pos_neg_placeholder() {}

#[test]
#[ignore = "TODO: sliding-window algorithm not implemented in actant-reliability::throttle"]
fn sliding_window_pos_neg_placeholder() {}

#[test]
#[ignore = "TODO: concurrency-semaphore algorithm not implemented in actant-reliability::throttle"]
fn concurrency_pos_neg_placeholder() {}

#[test]
#[ignore = "TODO: weighted-fair-queue algorithm not implemented in actant-reliability::throttle"]
fn wfq_pos_neg_placeholder() {}

#[test]
#[ignore = "TODO: priority queue algorithm not implemented in actant-reliability::throttle"]
fn priority_pos_neg_placeholder() {}

#[test]
#[ignore = "TODO: deadline queue algorithm not implemented in actant-reliability::throttle"]
fn deadline_pos_neg_placeholder() {}

#[test]
#[ignore = "TODO: adaptive (provider-header driven) algorithm not implemented in actant-reliability::throttle"]
fn adaptive_pos_neg_placeholder() {}
