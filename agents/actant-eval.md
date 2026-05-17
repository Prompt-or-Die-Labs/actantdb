# Work package: `actant-eval`

## Context

Eval-case runtime. Phase 4. Reads `eval_case`, runs a replay under the eval's constraints, writes `eval_run` rows.

## Specs to read first

- `/specs/13-actant-contract.md` §14 (replay as loop closure).
- `/specs/14-extended-primitives.md` §7 (eval_case + eval_run).
- `/specs/07-workflows-and-replay.md` (replay modes).
- `/specs/adr/0006-regret-hooks.md`.

## Scope

```rust
pub struct EvalRunner { /* depends on actant-replay, actant-policy, actant-storage */ }

impl EvalRunner {
    pub async fn run(&self, eval_case_id: &EvalCaseId) -> Result<EvalRunRecord, EvalError>;
    pub async fn create_from_replay(&self, tx: &mut Transaction<'_>, replay_run_id: &ReplayRunId, spec: EvalSpec) -> Result<EvalCaseId, EvalError>;
    pub async fn enable(&self, id: &EvalCaseId) -> Result<(), EvalError>;
    pub async fn disable(&self, id: &EvalCaseId) -> Result<(), EvalError>;
}

pub struct EvalSpec {
    pub name: String,
    pub expected_behavior: String,
    pub forbidden_behavior: Option<String>,
    pub success_criteria: SuccessCriteria,
    pub policy_constraints: Option<String>,
}
```

### Success criteria DSL (Phase 4 v1)

```jsonc
// SuccessCriteria
{
  "all_of": [
    { "must_emit":     "tool_call_finished" },
    { "must_not_emit": "policy_blocked" },
    { "cost_le":       0.05 },
    { "latency_le_ms": 5000 },
    { "assert":        { "jsonpath": "$.events[?(@.type=='model_call_finished')][0].output_tokens",
                         "op": "lt", "value": 800 } }
  ]
}
```

Versioned via the artifact pointer in `eval_case.success_criteria`; older versions remain readable.

### Internal modules

```
crates/actant-eval/src/
├── lib.rs
├── runner.rs
├── dsl.rs                  // SuccessCriteria parsing + evaluation
├── from_replay.rs
└── error.rs
```

### Tests

- Round-trip: a known passing replay produces `passed=1` against an expected-emit criterion.
- A criterion that mentions a non-existent jsonpath produces `failure_detail_ref` not a panic.
- Orphan detection: deleting the underlying checkpoint flips the eval to `enabled=0`.
- `create_from_replay` mints an `eval_case` whose checkpoint resolves and whose first run reproduces the original outcome.

## Acceptance criteria

- [ ] Build / test / clippy green.
- [ ] Every operator in the v1 DSL has a positive + negative test.
- [ ] Running an eval against a 1000-event session completes in under 30s on a laptop with `mode=recorded`.

## Do NOT

- Do NOT extend `actant-replay`'s loops; you are a consumer.
- Do NOT mutate non-eval rows from inside the runner.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
