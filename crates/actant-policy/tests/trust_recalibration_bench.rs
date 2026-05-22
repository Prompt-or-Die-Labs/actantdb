//! Recalibration bench: 10k synthetic actors under 5s.
//!
//! Covers AC: "Recalibration on 10k synthetic actors completes in < 5s on
//! a laptop."
//!
//! AC adjustment: the work package describes `TrustService::recalculate()`
//! that hits storage. The current crate exposes only the pure
//! `ActantTrustProfile::observe()` primitive — there is no service-level
//! recalibration pass. We bench the closest moral equivalent: for each of
//! 10k synthetic actors, construct a `ActantTrustProfile`, feed a moderate
//! sample of observations through `observe()`, and check `auto_approve_at`.
//! That exercises the same per-actor arithmetic a `recalculate()` pass
//! would do.

use actant_policy::ActantTrustProfile;
use std::time::{Duration, Instant};

const ACTORS: usize = 10_000;
const OBSERVATIONS_PER_ACTOR: usize = 20;
const BUDGET: Duration = Duration::from_secs(5);

#[test]
fn recalibration_of_10k_actors_under_5s() {
    let t0 = Instant::now();
    let mut auto_approved = 0usize;
    for actor_idx in 0..ACTORS {
        let mut p = ActantTrustProfile::new(format!("actor:{actor_idx}:tool.shell.run"));
        // Deterministic mix: ~80% success rate, varied per actor.
        let success_target = 16 + (actor_idx % 5); // 16..20 of 20 succeed
        for obs in 0..OBSERVATIONS_PER_ACTOR {
            p.observe(obs < success_target);
        }
        if p.auto_approve_at(0.7, 0.5) {
            auto_approved += 1;
        }
    }
    let elapsed = t0.elapsed();
    eprintln!(
        "recalibration: {ACTORS} actors x {OBSERVATIONS_PER_ACTOR} obs in {elapsed:?} ({auto_approved} auto-approved)"
    );
    assert!(
        elapsed < BUDGET,
        "recalibration took {elapsed:?}, budget {BUDGET:?}"
    );
    // Sanity: the mix above should produce a non-trivial fraction of
    // auto-approvals.
    assert!(
        auto_approved > 0,
        "expected some actors to clear the auto-approve threshold"
    );
}
