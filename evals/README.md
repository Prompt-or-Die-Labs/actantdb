# Evals

Seed evaluation corpus for the ActantDB DSL (`@actantdb/types::SuccessCriteria`
+ `actant-eval::Criterion`).

Each `seed/*.json` file describes one eval case:
- `id` — stable identifier.
- `name` — display name.
- `expected_behavior` — natural-language description of the success case.
- `forbidden_behavior` — natural-language description of what counts as failure.
- `success_criteria` — programmatic checks (DSL: `must_emit`, `must_not_emit`,
  `cost_le`, `latency_le_ms`, `assert.jsonpath` — see `crates/actant-eval/src/dsl.rs`).

Run the seed corpus with:

```bash
cargo run -p actant-eval --example run_seed -- evals/seed
```

Add a new case: drop a JSON file in `seed/`. Don't edit existing ones — they're
the regression catalog and renaming a key is a behavior change.
