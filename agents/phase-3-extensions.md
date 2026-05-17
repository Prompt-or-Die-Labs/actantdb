# Phase 3 extensions to existing crates

## Context

Consumed alongside `actant-embed.md`, `actant-capsule.md`, `actant-trust.md`. This is the multi-crate companion to Phase 3's new libraries — it catalogs what must change in each existing crate (`actant-storage`, `actant-context`, `actant-memory`, `actant-policy`, `actant-command`, `actant-subscribe`, `actant-server`) to land the six-stage context pipeline, full memory lifecycle, sensitivity lineage via capsules, memory conflicts, behavioral trust reads, and context-debt scoring.

## Scope

Each per-crate section below is independently executable but must all land in the Phase 3 release. Work one crate at a time, in the dependency order shown in `/agents/README.md`.


## Specs to read first

- `/specs/06-context-and-memory.md` — full file.
- `/specs/14-extended-primitives.md` §3 (capsule), §8 (memory conflict), §10 (trust), §13 (context debt).
- `/specs/adr/0005-data-capsules.md`, `/specs/adr/0007-behavioral-trust.md`.
- `/planning/phase-3-plan.md`.

## Per-crate work

### `actant-storage`

Row mappers and Transaction methods for `capsule`, `capsule_membership`, `memory_conflict`, `trust_profile`, `context_debt`, `model_route_decision`. Helpers:

- `attach_memberships_for_object` — used by derivations.
- `list_active_memories(workspace, scope)` — for context gather.
- `insert_memory_conflict`, `set_conflict_resolution`, `resolve_conflict`.
- `insert_or_update_trust_profile`.
- `insert_context_debt`.

### `actant-context`

Implement the six-stage pipeline fully:

1. **Gather** — pull recent messages (window 50), active memories, system prompts, attached files, recent tool results, workflow state.
2. **Score** — pluggable trait; default scorer = weighted (recency × similarity × pin × purpose-match × prior-use). Similarity comes from `actant-embed` queries.
3. **Firewall** — capsule policy + per-row sensitivity + visibility intersected with the model_route's requirements.
4. **Redact** — secret-pattern, PII pattern, path normalization, injection wrapping.
5. **Truncate** — greedy by rank within `token_budget`; pinned items never truncated.
6. **Emit** — `context_build` + `context_item` rows + `context_debt` row.

Pluggable scorer: `pub trait ContextScorer: Send + Sync { fn score(&self, item: &Candidate, ctx: &ScoringContext) -> f32; }`.

### `actant-memory`

Add lifecycle commands implementations:

- `edit_memory`, `restrict_memory`, `expire_memory`, `revoke_memory`, `delete_memory`.
- Embedding cascade: approval triggers a `memory.embed` effect; deletion removes the embedding via `actant-embed::delete`.
- Conflict detection: a post-approval check scans for `conflict_type` against existing memories (text overlap heuristic; pluggable).
- Provenance traversal helper: `provenance(memory_id) -> Provenance` walks back to source events through `memory_candidate.source_event_ids`.
- Honor `allowed_contexts` / `forbidden_contexts` / `last_verified_at` in retrieval.

### `actant-policy`

- Read `trust_profile` when computing risk for an actor; low trust escalates `medium → high` (configurable threshold).
- Consult capsule policy via `actant-capsule::resolve` + `compose_strictest` when evaluating effects against context items.

### `actant-command`

New commands: `create_capsule`, `attach_to_capsule`, `retire_capsule`, `detect_memory_conflict`, `set_conflict_resolution`, `resolve_conflict`, `recalculate_trust`, `pin_trust`, plus the new memory lifecycle commands.

### `actant-subscribe`

New targets: `capsule`, `memory_conflict`, `trust_profile`, `context_debt`, `model_route_decision`.

### `actant-server`

Endpoints for new commands; metadata for new tables.

## Acceptance criteria

- [ ] Phase 3 decision gate passes (`/planning/phase-3-plan.md` §"Decision gate").
- [ ] Sensitivity-lineage property tests pass (100 random source/derivation chains).
- [ ] `delete_memory` removes both row text and embedding atomically.
- [ ] A low-trust actor's previously auto-approved tool call now requires approval — verified by a fixture session.

## Do NOT

- Do NOT contradict the strictest-wins rule on capsule composition.
- Do NOT let trust grant authority. Advisory only.
- Do NOT add embedding logic outside `actant-embed`.
