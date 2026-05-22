//! AC for the model registry — "Selection is deterministic for the same
//! inputs (modulo randomized tie-break which is seeded)."
//!
//! The Phase 1 `ActantModelRegistry` exposes `pick_cheapest_cloud(privacy_class)` and
//! `pick_local()`. Neither accepts an RNG seed; cloud routing breaks cost ties by
//! provider and model name, while local routing uses `Iterator::min_by_key`.
//!
//! This test asserts that 1000 successive calls against a registry populated
//! with three equally-priced cloud models always return the same model — i.e.
//! the routing path holds no hidden non-determinism (no time-based jitter, no
//! HashMap iteration order, no `rand::random()` slipping in). When the router
//! grows a seedable RNG for explicit tiebreaks, the test should be extended
//! to assert (a) seed-equal calls match and (b) different seeds reshuffle the
//! distribution — for now there is no RNG to exercise, so only (a) applies.

use actant_command::models::{ActantModelInfo, ActantModelRegistry};

fn equally_priced(name: &str) -> ActantModelInfo {
    ActantModelInfo {
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
    let mut r = ActantModelRegistry::default();
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
    let mut r = ActantModelRegistry::default();
    for name in ["mlx-a", "mlx-b", "mlx-c"] {
        r.register(ActantModelInfo {
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

#[test]
fn cloud_cost_tiebreak_uses_provider_then_name() {
    let mut r = ActantModelRegistry::default();
    r.register(ActantModelInfo {
        provider: "vendor-b".into(),
        name: "alpha".into(),
        ..equally_priced("alpha")
    });
    r.register(ActantModelInfo {
        provider: "vendor-a".into(),
        name: "zeta".into(),
        ..equally_priced("zeta")
    });
    r.register(ActantModelInfo {
        provider: "vendor-a".into(),
        name: "beta".into(),
        ..equally_priced("beta")
    });
    let pick = r.pick_cheapest_cloud("public").unwrap();
    assert_eq!(pick.provider, "vendor-a");
    assert_eq!(pick.name, "beta");
}

#[test]
fn pick_cheapest_cloud_ignores_non_finite_costs() {
    let mut r = ActantModelRegistry::default();
    let mut nan = equally_priced("nan");
    nan.cost_per_output_1k = f64::NAN;
    r.register(nan);
    r.register(equally_priced("finite"));

    let pick = r.pick_cheapest_cloud("public").unwrap();
    assert_eq!(pick.name, "finite");
}
