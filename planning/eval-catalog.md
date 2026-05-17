# Eval catalog — seed eval cases for each phase

A catalog of eval cases that ship with the system. Each is minted from a constructed failure scenario; together they form the "regression suite" for the Actant Contract.

## Conventions

- Eval cases are stored as JSON files under `evals/seed/`.
- Each file points to a fixture checkpoint that ships with the repo (under `tests/fixtures/checkpoints/`).
- The success criteria use the DSL in `/agents/actant-eval.md`.
- Coding agents use these as the spec for the Eval Catalog screen in Studio.

## Catalog

### Phase 2 seeds

| Name                                | Asserts                                                                | Source         |
| ----------------------------------- | ---------------------------------------------------------------------- | -------------- |
| `shell_destructive_blocked`         | `rm -rf` proposal triggers `intent_action_mismatch`, blocks effect.    | constructed    |
| `shell_dangerous_requires_approval` | `pip install` proposal produces `tool_call_pending_approval`.          | constructed    |
| `file_write_capture_pre_state`      | File-write effect has a non-null `compensation_plan.pre_state_artifact_ref`. | constructed |
| `lease_loss_recovery`               | Killed worker mid-effect → re-claim → same final outcome.              | constructed    |
| `idempotency_no_double_apply`       | Re-claim of an idempotent effect returns the original `effect_result`.  | constructed    |
| `drift_score_crosses_threshold`     | Five consecutive intent-mismatches escalates risk on the next request.  | constructed    |
| `budget_halt`                       | An agent hitting its `tool_calls` budget halts; further requests denied.| constructed    |
| `delegation_authority_bounded`      | Child subagent cannot exercise authority outside the delegation set.    | constructed    |
| `intervention_pause_resume`         | A `pause` intervention freezes the session; resume produces a new event.| constructed    |

### Phase 3 seeds

| Name                                | Asserts                                                                |
| ----------------------------------- | ---------------------------------------------------------------------- |
| `cloud_blocked_for_high_sensitivity`| A context build with a `high`-sensitivity item targeting cloud route blocks the item, builds remainder. |
| `capsule_lineage_strict`            | Derivation inherits the strictest source capsule policy.               |
| `memory_revoke_removes_embedding`   | `revoke_memory` deletes the embedding; future queries don't see it.    |
| `memory_conflict_resolution`         | Conflict with `newer_wins` policy returns newer only.                  |
| `trust_low_escalates_risk`           | Actor with `trust_profile.score < 0.4` in `shell.run` triggers approval. |
| `context_debt_warning`               | A build with debt score > workspace threshold emits a warning.         |

### Phase 4 seeds

| Name                                | Asserts                                                                |
| ----------------------------------- | ---------------------------------------------------------------------- |
| `workflow_pause_resume_across_restart`| Workflow `waiting_human` survives a 10-min process restart.         |
| `workflow_retry_then_succeed`       | Step failing twice then succeeding terminates `succeeded`.             |
| `approval_gate_within_workflow`     | Workflow halts at `approval_gate` until human approves.                |
| `regret_demote_memory`              | `resolve_regret` with corrective `demote_memory` reduces that memory's `usage_count` priority. |
| `eval_from_replay_round_trip`       | `create_eval_from_replay` + `run_eval` reproduces the original outcome.|

### Phase 5 seeds

| Name                                | Asserts                                                                |
| ----------------------------------- | ---------------------------------------------------------------------- |
| `recorded_replay_identical`         | `mode=recorded` produces `replay_diff.kind='identical'` for every event.|
| `policy_replay_branches`             | Stricter policy blocks one branch; diff records the missing event.    |
| `memory_replay_changes_proposal`    | Excluding a memory changes the planner's tool proposal.                |
| `local_only_no_fallback_halts`      | A cloud-only step in `local_only` mode halts; diff entry recorded.     |

### Phase 6 seeds

| Name                                | Asserts                                                                |
| ----------------------------------- | ---------------------------------------------------------------------- |
| `cross_workspace_blocked`           | A command targeting another workspace without `cross_workspace.*` denied. |
| `sync_local_only_does_not_leak`     | Capsule `local_only` content absent from sync destination.             |
| `audit_export_byte_identical_rerun` | Re-run with same parameters produces byte-identical files.             |
| `retention_tombstones_payload`      | Past-retention events appear with payload tombstoned, hash intact.     |

## Adding a new eval

1. Construct or capture a scenario as a fixture checkpoint under `tests/fixtures/checkpoints/`.
2. Write the eval JSON under `evals/seed/`.
3. Reference it from this file under the correct phase.
4. Add the eval-case row in the workspace bootstrap migration so Studio sees it on first run.

## Cadence

- Phase 2-onward: every seed eval runs on every PR.
- Phase 4-onward: scheduled nightly runs against a representative production-like workspace.
- Failures route to the Regret Inbox.
