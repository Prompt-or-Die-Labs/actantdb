//! Latency benchmark: p99 `try_consume()` must be <= 1ms.
//!
//! Covers AC: "p99 `check()` latency <= 1ms in the bench harness."
//!
//! The work package describes a `ThrottleService::check()` API. The current
//! crate exposes the underlying `Bucket::try_consume()` token-bucket
//! primitive. We bench that primitive (which is what any `check()` call
//! would dispatch into); document the AC adjustment in the test header.

use actant_throttle::{Bucket, Policy};
use std::time::{Duration, Instant};

const ITERS: usize = 10_000;
const P99_BUDGET: Duration = Duration::from_millis(1);

#[test]
fn p99_try_consume_latency_under_1ms() {
    // Bucket sized so most consumes succeed and we measure the fast path;
    // a small fraction of failures is also fast (rejection arithmetic).
    let mut b = Bucket::new(Policy {
        limit: 10_000,
        refill_per_second: 100_000.0,
    });

    // Warm-up — first call may take longer due to clock/page faults.
    for _ in 0..1000 {
        let _ = b.try_consume(1);
    }

    let mut latencies: Vec<Duration> = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        let _ = b.try_consume(1);
        latencies.push(t0.elapsed());
    }

    latencies.sort_unstable();
    let p50 = latencies[ITERS / 2];
    let p99 = latencies[9900]; // 99th percentile of 10k samples
    let p999 = latencies[9990];
    let max = *latencies.last().unwrap();

    eprintln!(
        "try_consume latencies (n={ITERS}): p50={p50:?} p99={p99:?} p999={p999:?} max={max:?}"
    );
    assert!(
        p99 <= P99_BUDGET,
        "p99 {p99:?} exceeded budget {P99_BUDGET:?} (p50={p50:?}, max={max:?})"
    );
}
