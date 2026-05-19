//! AC for the model registry — "Selection is deterministic for the same
//! inputs (modulo randomized tie-break which is seeded)."
//!
//! The Phase 1 `Registry` exposes `pick_cheapest_cloud(privacy_class)` and
//! `pick_local()`. Neither accepts an RNG seed; both rely on the stable
//! tiebreak provided by `Iterator::min_by` / `min_by_key`, which returns the
//! **first** element among equals (Rust 1.34+ behaviour). That mechanical
//! tiebreak is what "determinism" means for this crate today: same registry
//! contents + same input → same model, every time.
//!
//! This test asserts that 1000 successive calls against a registry populated
//! with three equally-priced cloud models always return the same model — i.e.
//! the routing path holds no hidden non-determinism (no time-based jitter, no
//! HashMap iteration order, no `rand::random()` slipping in). When the router
//! grows a seedable RNG for explicit tiebreaks, the test should be extended
//! to assert (a) seed-equal calls match and (b) different seeds reshuffle the
//! distribution — for now there is no RNG to exercise, so only (a) applies.

use actant_runtime::models::{ModelInfo, Registry};

fn equally_priced(name: &str) -> ModelInfo {
    ModelInfo {
        provider: "vendor".into(),
        name: name.into(),
        locality: "cloud".into(),
        privacy_class: "public".into(),
        cost_per_input_1k: 0.001,
        cost_per_output_1k: 0.005,
        latency_p50_ms: 400,
    }
}

#[test]
fn pick_cheapest_cloud_is_deterministic_under_ties() {
    let mut r = Registry::default();
    r.register(equally_priced("alpha"));
    r.register(equally_priced("beta"));
    r.register(equally_priced("gamma"));

    let first = r
        .pick_cheapest_cloud("public")
        .expect("registry has cloud models")
        .name
        .clone();

    for i in 0..1000 {
        let pick = r
            .pick_cheapest_cloud("public")
            .expect("registry has cloud models");
        assert_eq!(
            pick.name, first,
            "iteration {i}: tiebreak changed (got {} expected {})",
            pick.name, first
        );
    }
}

#[test]
fn pick_local_is_deterministic_under_ties() {
    let mut r = Registry::default();
    for name in ["mlx-a", "mlx-b", "mlx-c"] {
        r.register(ModelInfo {
            provider: "mlx".into(),
            name: name.into(),
            locality: "local".into(),
            privacy_class: "private".into(),
            cost_per_input_1k: 0.0,
            cost_per_output_1k: 0.0,
            latency_p50_ms: 200,
        });
    }
    let first = r.pick_local().expect("has local model").name.clone();
    for _ in 0..1000 {
        let pick = r.pick_local().expect("has local model");
        assert_eq!(pick.name, first);
    }
}

/// Insertion-order documents the observable tiebreak. `Iterator::min_by_key`
/// returns the **first** element when keys tie. If a future router replaces
/// the strategy (e.g. last-wins, or a hash-of-name lexicographic shuffle),
/// this test fails loudly and forces an explicit decision rather than a
/// silent behavioural change.
#[test]
fn min_by_key_tiebreak_returns_first_inserted() {
    let mut r = Registry::default();
    r.register(equally_priced("first"));
    r.register(equally_priced("second"));
    r.register(equally_priced("third"));
    let pick = r.pick_cheapest_cloud("public").unwrap();
    assert_eq!(
        pick.name, "first",
        "Registry tiebreak no longer returns first-inserted; update this test \
         after confirming the new policy is intentional."
    );
}
