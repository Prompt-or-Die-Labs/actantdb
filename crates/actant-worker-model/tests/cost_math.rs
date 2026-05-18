//! Cost-math AC: "Cost math matches the documented `model_route` rates to
//! within 1e-6."
//!
//! We don't need a live provider call to verify the formula — the formula is
//! a pure function over `(tokens_in, tokens_out, rates)`. The Mock provider
//! is used in the round-trip test below to confirm the Handler integrates
//! the same numbers without side effects.

use actant_worker_model::{compute_cost_usd, CostRates, ModelHandler};
use actant_worker_protocol::Handler;

const EPSILON: f64 = 1e-6;

/// Headline case from the task: $1.00 per 1k input, $2.00 per 1k output,
/// 750 input + 250 output = $0.75 + $0.50 = **$1.25**.
#[test]
fn cost_for_task_example_matches_within_epsilon() {
    let rates = CostRates {
        input_per_1k: 1.00,
        output_per_1k: 2.00,
    };
    let cost = compute_cost_usd(750, 250, rates);
    assert!(
        (cost - 1.25).abs() < EPSILON,
        "expected 1.25 within {EPSILON}, got {cost}"
    );
}

/// Spec-table case from `agents/actant-worker-model.md` Tests section:
/// `cost_per_input_1k=$0.01, cost_per_output_1k=$0.03` × 1000/500 = $0.025.
#[test]
fn cost_for_spec_table_example_matches_within_epsilon() {
    let rates = CostRates {
        input_per_1k: 0.01,
        output_per_1k: 0.03,
    };
    let cost = compute_cost_usd(1000, 500, rates);
    assert!(
        (cost - 0.025).abs() < EPSILON,
        "expected 0.025 within {EPSILON}, got {cost}"
    );
}

#[test]
fn cost_is_zero_for_zero_tokens() {
    let rates = CostRates {
        input_per_1k: 1.0,
        output_per_1k: 2.0,
    };
    assert!(compute_cost_usd(0, 0, rates).abs() < EPSILON);
}

#[test]
fn cost_scales_linearly_with_tokens() {
    let rates = CostRates {
        input_per_1k: 1.0,
        output_per_1k: 2.0,
    };
    let single = compute_cost_usd(100, 100, rates);
    let triple = compute_cost_usd(300, 300, rates);
    assert!(
        (triple - 3.0 * single).abs() < EPSILON,
        "linearity broken: 3 * {single} != {triple}"
    );
}

#[test]
fn cost_distinguishes_input_from_output() {
    let asymmetric = CostRates {
        input_per_1k: 1.0,
        output_per_1k: 100.0,
    };
    let in_heavy = compute_cost_usd(1000, 0, asymmetric);
    let out_heavy = compute_cost_usd(0, 1000, asymmetric);
    assert!((in_heavy - 1.0).abs() < EPSILON);
    assert!((out_heavy - 100.0).abs() < EPSILON);
}

/// End-to-end: Mock provider returns token counts; apply the cost formula
/// to its reported numbers and confirm we still match the documented math.
/// This is the "use the Mock provider so no network call" branch of the AC.
#[tokio::test]
async fn mock_handler_token_counts_feed_cost_formula() {
    let handler = ModelHandler::mock();
    // The Mock provider's token-count heuristic is `prompt.len() / 4` (in) and
    // `(prompt.len() / 8).max(4)` (out). For a 16-byte prompt:
    //   tokens_in  = 16 / 4 = 4
    //   tokens_out = max(16 / 8, 4) = 4
    let prompt = "0123456789abcdef"; // 16 bytes
    let result = handler
        .handle(serde_json::json!({"prompt": prompt, "model": "mock"}))
        .await
        .unwrap();
    let tokens_in = result["tokens_in"].as_u64().unwrap() as u32;
    let tokens_out = result["tokens_out"].as_u64().unwrap() as u32;
    assert_eq!(tokens_in, 4);
    assert_eq!(tokens_out, 4);

    let rates = CostRates {
        input_per_1k: 1.00,
        output_per_1k: 2.00,
    };
    // (4/1000)*1 + (4/1000)*2 = 0.004 + 0.008 = 0.012
    let cost = compute_cost_usd(tokens_in, tokens_out, rates);
    assert!(
        (cost - 0.012).abs() < EPSILON,
        "expected 0.012 within {EPSILON}, got {cost}"
    );
}
