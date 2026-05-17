# Phase 5 extensions to existing crates

## Context

Phase 5 is concentrated in `actant-replay`. This work package catalogs the small additions to other crates that the replay loops require: replay-scoped effect mode, deterministic context-build replay, policy replay entry-point, replay route in the server.

## Scope

`actant-replay` is the primary destination; everything else is a small targeted change. Work `actant-replay` first, then the supporting crates in parallel.


## Specs to read first

- `/specs/07-workflows-and-replay.md` §§6–10.
- `/specs/14-extended-primitives.md` §13 (context debt — refined here).
- `/specs/14-extended-primitives.md` §14 (drift threshold gating — refined here).
- `/planning/phase-5-plan.md`.

## Per-crate work

### `actant-replay` (primary)

Implement all seven replay modes:

- `recorded` — replay using stored `effect_result` and `model_call.response_ref`.
- `experimental` — re-invoke models and tools via replay-scoped worker fleet.
- `policy` — re-evaluate Guard with `overrides.policy_id`.
- `model` — alternate `model_route_id`.
- `memory` — apply `exclude_memory_ids` / `edit` overrides.
- `tool` — mocked tool results from `overrides.mock_tools`.
- `local_only` — forbid cloud routes; halt branches without local fallback.

Implement the replay event loop in `loop.rs`. Synthetic events live in the replay scope; do not write to `agent_event`.

Implement `diff` producer with kinds `identical | changed | missing | extra`. Implement `summary` aggregator that produces the high-level diff categories listed in `/specs/07-workflows-and-replay.md` §9.

### `actant-policy`

`policy` replay mode entry point: a fresh evaluator instance backed by the override policy.

### `actant-effects`

A "replay-scope" mode flag on the effect queue: claims and completions in this mode write to replay-scoped state, not the main projection rows. Workers running in replay accept a `replay_run_id` in the lease and disable any side-effect compensation outside the replay scope.

### `actant-context`

Phase 5 hardens deterministic reproducibility of `build_context`. Where any non-determinism exists (LRU caches, random tie-break), make it seedable; the seed is part of the replay-scope state.

### `actant-server`

`POST /v1/replay`, `GET /v1/replay/{id}` return real results (Phase 1 stubbed them).

## Acceptance criteria

- [ ] Three named replay scenarios from `/specs/10-alpha-demo.md` §12 produce diffs rendered by Studio.
- [ ] A failing eval re-runs from its checkpoint with a deterministic diff in `mode=recorded`.
- [ ] Replay isolation property test: no row in the main projection set is written during a replay.
- [ ] Snapshot purging an old checkpoint correctly orphans dependent eval cases (`eval_case.enabled=0` and a `eval_case_orphaned` event).

## Do NOT

- Do NOT write to `agent_event` from inside replay. Use replay-scoped artifacts.
- Do NOT make `experimental` mode silently re-call cloud models if the replay was launched in `local_only` mode by composition.
- Do NOT skip determinism seeds. Replay reproducibility is the property we're shipping.
