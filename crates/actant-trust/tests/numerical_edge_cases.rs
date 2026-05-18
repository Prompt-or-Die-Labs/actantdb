//! Numerical edge cases: no public function panics on degenerate input.
//!
//! Covers AC: "No public function panics on bad signals (e.g. NaN,
//! sample_size=0); returns `(0.0, 0.0)` with low confidence."
//!
//! AC adjustment: the work package describes `TrustService::compute(&Signals)
//! -> (f32, f32)` returning `(0.0, 0.0)` for sample_size=0. The current
//! crate (see `crates/actant-trust/src/lib.rs`) instead exposes the
//! `TrustProfile { new, observe, auto_approve_at }` API:
//! - cold-start `TrustProfile::new(area)` returns `score=0.5, confidence=0.0,
//!   sample_size=0` (the "low-confidence" guard the AC asks for, but the
//!   score baseline is 0.5 not 0.0).
//!
//! We test the panic-safety guarantee that IS testable on the current
//! API and add an ignored placeholder for the `(0.0, 0.0)` invariant
//! once `compute()` lands.

use actant_trust::TrustProfile;

#[test]
fn cold_start_sample_size_is_zero_and_confidence_is_zero() {
    let p = TrustProfile::new("tool:test.cold");
    assert_eq!(p.sample_size, 0);
    assert_eq!(p.confidence, 0.0);
    // Current crate semantics: cold-start score baseline is 0.5 (neutral
    // prior). The work-package AC expects (0.0, 0.0) under
    // `TrustService::compute()` once that API lands.
    assert_eq!(p.score, 0.5);
    // Cold profile must NEVER auto-approve.
    assert!(!p.auto_approve_at(0.0, 0.0001));
}

#[test]
fn nan_outcome_loop_does_not_panic() {
    // `observe()` takes a bool, so callers cannot directly feed NaN, but
    // we exercise the closest moral equivalent: a long alternating
    // sequence of success / failure with no early termination must not
    // panic, must keep `score` in [0, 1], and `confidence` in [0, 1].
    let mut p = TrustProfile::new("tool:test.alternating");
    for i in 0..10_000 {
        p.observe(i % 2 == 0);
        assert!(p.score.is_finite(), "score went non-finite at i={i}");
        assert!(
            p.confidence.is_finite(),
            "confidence went non-finite at i={i}"
        );
        assert!(
            (0.0..=1.0).contains(&p.score),
            "score escaped [0,1] at i={i}: {}",
            p.score
        );
        assert!(
            (0.0..=1.0).contains(&p.confidence),
            "confidence escaped [0,1] at i={i}: {}",
            p.confidence
        );
    }
}

#[test]
fn all_failures_drives_score_to_zero_without_panic() {
    let mut p = TrustProfile::new("tool:test.fail");
    for _ in 0..1000 {
        p.observe(false);
    }
    assert!(p.score.is_finite());
    assert!(
        p.score < 0.01,
        "expected score near 0 after 1k failures, got {}",
        p.score
    );
    assert!(!p.auto_approve_at(0.5, 0.5));
}

#[test]
fn all_successes_drives_score_to_one_without_panic() {
    let mut p = TrustProfile::new("tool:test.win");
    for _ in 0..1000 {
        p.observe(true);
    }
    assert!(p.score.is_finite());
    assert!(
        p.score > 0.99,
        "expected score near 1 after 1k successes, got {}",
        p.score
    );
}

#[test]
#[ignore = "TODO: TrustService::compute(&Signals) -> (f32, f32) per work package not yet implemented; once it lands, this test should assert compute(&Signals{ sample_size: 0, .. }) == (0.0, 0.0) and that NaN-bearing Signals do not panic."]
fn compute_handles_zero_sample_and_nan_signals() {
    // When the spec-13 `Signals` struct + `compute()` arrive, restore this
    // body:
    //
    // let mut s = Signals::default();
    // s.sample_size = 0;
    // assert_eq!(TrustService::compute(&s), (0.0, 0.0));
    //
    // let mut s_nan = Signals::default();
    // s_nan.tool_success_rate = f32::NAN;
    // s_nan.sample_size = 100;
    // let (score, conf) = TrustService::compute(&s_nan);
    // assert!(score.is_finite() && conf.is_finite());
}
