# Phase 5 plan — Replay engine

## Goal

Implement all seven replay modes from `/specs/07-workflows-and-replay.md` §6 with a diff viewer that turns "is this fixed?" into a query.

When Phase 5 ends, the three named replay scenarios from the alpha demo (`memory`, `policy`, `experimental`) reproduce deterministically (within model-determinism limits) and produce a diff that Studio renders.

## Duration

6–8 weeks.

## New crates introduced

| Crate                | Kind | Purpose                                                |
| -------------------- | ---- | ------------------------------------------------------ |
| (none — Phase 5 is all in `actant-replay`) |

Replay-scoped synthetic event storage is in artifacts in Phase 5; if performance demands move it into a dedicated table, that lives in a Phase 5 ADR.

## Existing crates expanded

| Crate            | Phase 5 expansion                                                          |
| ---------------- | -------------------------------------------------------------------------- |
| `actant-replay`  | Phase 1 wrote checkpoints; Phase 5 implements every replay mode and the diff producer. |
| `actant-policy`  | `policy` replay mode reads `overrides.policy_id` and re-evaluates Guard. |
| `actant-context` | `memory` and `policy` replays re-derive context builds; Phase 5 hardens the deterministic reproducibility of the build pipeline. |
| `actant-effects` | `experimental` replay re-enqueues effects in a replay scope; workers in replay mode are configured per the override. |
| `actant-command` | Replay scope: synthetic events do not write to the real `agent_event` table; instead they go to artifacts (or, by ADR, a dedicated table). Command-engine helpers gate this. |
| `actant-server`  | `POST /v1/replay`, `GET /v1/replay/{id}` now return real results. `POST /v1/checkpoints` already shipped. |
| `actant-subscribe` | `replay_run` and `replay_diff` subscriptions become primary Studio data sources. |

## Replay modes — implementation notes

### `recorded` (no model/tool re-invocation)

Read `effect_result` and `model_call.response_ref` by their stored IDs. Replay produces synthetic events at the same timestamps shifted into the replay's time origin.

### `experimental` (re-invoke models + tools)

Replay-scoped worker fleet: the worker binary is the same as production, but the lease points to a replay-scoped effect ID, and the worker is told `mode=experimental` so it does not write to user-facing artifacts.

### `policy` (alternate `policy_id`)

For every Guard decision in the original run, re-evaluate under `overrides.policy_id`. Where the decision differs, branch — original `allow`, replay `deny` produces a diff; original `deny`, replay `allow` requires that the recorded `effect_result` be present (because replay does not perform new side effects in this mode).

### `model` (alternate `model_route_id`)

Models re-invoke under the alternate route. Tools reuse recorded results. The diff highlights different `model_call.response_ref` and any downstream tool argument changes.

### `memory` (excluded / edited memories)

Replays the context build with the memory override applied. Model and tools reuse recorded results, so the diff isolates the *prompt change* and shows whether the model would have proposed differently.

### `tool` (mocked tools)

Tool calls return the override-specified mock results (or read-only stubs). Useful for debugging "what would the agent do if pytest had passed instead of failed?"

### `local_only` (cloud routes forbidden)

A combination override: any cloud route in the original run is replaced with its local fallback. If no local fallback exists, the branch halts and the diff entry records `local_only_fallback_unavailable`.

## Studio additions (Phase 5)

- **Replay Lab** — pick a checkpoint, mode, overrides. Side-by-side diff viewer.
- **Diff Viewer** — events grouped by `kind` (`identical`, `changed`, `missing`, `extra`); per-event detail link; cost/latency comparison panel.
- **Eval ↔ Replay link** — failed eval runs link to the replay that produced the failure.
- **Regret ↔ Replay link** — every resolved regret can show a replay proving the corrective action prevents recurrence.

## Test strategy

- **Determinism harness.** Snapshot-replay round-trip for `mode=recorded` must produce a diff with `kind='identical'` for every event in a curated session.
- **Snapshot completeness.** Every replay mode is exercised against a 100-event session; missing snapshots produce `precondition_failed` rather than wrong results.
- **Replay isolation.** A replay run cannot write to non-replay rows. A grep of `INSERT INTO agent_event` from within replay-scope code returns nothing (architectural test).
- **Cost/latency panel.** Diff aggregates match per-event totals.

## Decision gate

Phase 5 passes when:

1. The three named replay scenarios from `/specs/10-alpha-demo.md` §12 run reproducibly and produce diffs that Studio renders.
2. A failing eval case re-runs from its checkpoint and produces a deterministic diff between original and replay.
3. Snapshot purging an old checkpoint correctly orphans dependent eval cases and replay runs.

## Risks

| Risk                                | Mitigation                                                                       |
| ----------------------------------- | -------------------------------------------------------------------------------- |
| Snapshot size                       | Phase 5 keeps snapshots in artifacts; if size is problematic, a delta-encoding ADR lands here. |
| Determinism limits in `experimental`| UI explicitly calls out which modes re-invoke models. Diffs grouped by event_type to surface noise vs signal. |
| Replay-scoped state storage         | Phase 5 starts with artifacts; performance review at end of phase decides whether a dedicated table is needed (ADR-0009 if so). |
| Cloud-route fallbacks               | `local_only` mode only works if local fallbacks are registered. Configuration responsibility lies with the workspace. |

## CLI deliverables (Phase 5)

Per `/planning/cli-design.md` § "CLI staging across phases":

- New subcommands: `actant replay diff <a> <b>`, `actant replay run ... --model|--tools|--without-memory|--policy`, `actant replay suite`, `actant studio replay <id> --open`.
- New examples: `replay-debugging` (deeper).

## Work packages

- `/agents/phase-5-extensions.md` — single phase doc; replay touches one crate primarily. Also extends `actant-cli` for the new replay subcommands.

## Sequencing

```
week 1-2
  ├── Replay loop skeleton + mode=recorded
  └── Diff producer with all four kinds (identical/changed/missing/extra)

week 3
  └── mode=memory + mode=tool + mode=policy

week 4-5
  ├── mode=experimental (replay-scoped worker fleet)
  ├── mode=model
  └── mode=local_only

week 6
  ├── Studio Replay Lab + Diff Viewer
  └── Eval ↔ Replay link

week 7-8 (buffer)
  └── Stabilization, performance, large-replay tests, decision gate
```
