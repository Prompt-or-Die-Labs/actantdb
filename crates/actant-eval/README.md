# actant-eval

Eval-case runtime. Phase 4.

Owns:

- The `EvalRunner` that takes an `eval_case` and:
  - resolves the checkpoint,
  - starts a replay under the eval's policy_constraints,
  - applies `success_criteria` (Phase 4 DSL: see below),
  - writes `eval_run` rows with pass/fail.
- A minimal success-criteria DSL: `must_emit`, `must_not_emit`, `final_event_equals`, `tool_arg_matches`, `cost_le`, `latency_le`, `assert(jsonpath, op, value)`. Versioned.
- Scheduled eval runs (cron via `actant-trigger`).
- `create_eval_from_replay` helper: snapshot a replay_run + expected/forbidden behaviors → mint an `eval_case`.

Does **not** own: replay loops (`actant-replay`). Eval is a *user* of replay.

See `agents/actant-eval.md` and `specs/adr/0006-regret-hooks.md`.
