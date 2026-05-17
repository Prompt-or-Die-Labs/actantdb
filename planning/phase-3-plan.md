# Phase 3 plan — Context engine + memory + capsules + trust

## Goal

Make context manifests, the firewall, and memory provenance the way they're described in `/specs/06-context-and-memory.md`. Add capsules (sensitivity lineage), memory-conflict detection, and the first version of behavioral trust profiles.

When Phase 3 ends, a context build can be inspected, a memory can be revoked or restricted, a derivation cannot accidentally drop a sensitivity label, and Guard can read trust scores when deciding whether to escalate risk.

## Duration

4 weeks.

## New crates introduced

| Crate            | Kind | Purpose                                                       |
| ---------------- | ---- | ------------------------------------------------------------- |
| `actant-embed`   | lib  | Adapter trait + bindings for vector stores: LanceDB (default), FAISS, Qdrant, Chroma, pgvector, SQLite-vec. Phase 3 ships LanceDB. |
| `actant-capsule` | lib  | Capsule resolution + strictest-wins composition + `attach_to_capsule` helpers. Used by the context engine, memory extractor, and artifact recorder. |
| `actant-trust`   | lib  | Trust profile computation and recalibration. Reads operational signals; emits `trust_*` events. |

`actant-conflict` (memory conflict detection) is added as a module inside `actant-memory` rather than a separate crate because its only consumer is the memory engine.

## Existing crates expanded

| Crate              | Phase 3 expansion                                                          |
| ------------------ | -------------------------------------------------------------------------- |
| `actant-context`   | Full six-stage pipeline (gather → score → firewall → redact → truncate → emit). Semantic similarity scorer powered by `actant-embed`. Capsule-aware firewall. Context-debt computation. |
| `actant-memory`    | Lifecycle commands: `edit_memory`, `restrict_memory`, `expire_memory`, `revoke_memory`, `delete_memory`. Embedding cascade via `actant-embed`. Conflict detection module. `record_memory_use` with provenance traversal. Memory `allowed_contexts` / `forbidden_contexts` / `last_verified_at` honored. |
| `actant-policy`    | Reads `trust_profile` to escalate risk when score is low. Capsule policy consulted during evaluation. |
| `actant-command`   | New commands: `create_capsule`, `attach_to_capsule`, `retire_capsule`, `detect_memory_conflict`, `set_conflict_resolution`, `resolve_conflict`, `recalculate_trust`, `pin_trust`. |
| `actant-storage`   | Row mappers for `capsule`, `capsule_membership`, `memory_conflict`, `trust_profile`, `context_debt`. |
| `actant-subscribe` | New subscription targets: `capsule`, `memory_conflict`, `trust_profile`, `context_debt`. |
| `actant-server`    | Endpoints for the new commands. Static `metadata/capsules` for Studio. |

## Specs landing in Phase 3

From `/specs/14-extended-primitives.md` §17:

- Capsule + sensitivity lineage
- Memory conflict + memory `allowed_contexts` / `forbidden_contexts` / `last_verified_at`
- Trust profile (initial scoring)

Plus the Phase 3 scope already named in `/specs/11-roadmap.md`:

- Context manifests fully populated
- Six-stage pipeline
- Default scorer (recency + similarity + pin signal)
- Default redaction passes
- Memory lifecycle commands
- Embedding workflow with at least one vector store

## Studio additions (Phase 3)

- **Context Inspector** — gathered/blocked items, capsule lineage, context-debt score with breakdown.
- **Memory Review** — provenance graph from memory back to source events; restrict / expire / revoke buttons; conflict surfaces with resolution choice.
- **Capsule Browser** — what capsules exist; what objects are in each; policy summary.
- **Trust panel** — per-actor, per-capability score trend.

## Test strategy

Phase-3-specific:

- **Sensitivity lineage property tests.** Random source-policy combinations followed by random derivations; assert the strictest policy wins at every join.
- **Capsule revocation cascade.** Retiring a capsule does not retroactively change downstream capsule_membership rows; new derivations stop inheriting; existing items keep their policy.
- **Conflict resolution.** With two contradictory approved memories and a `resolution_policy='newer_wins'`, the context engine includes only the newer one when both match a query.
- **Trust score stability.** Synthetic actor histories produce expected score curves; thresholds emit `trust_downgrade` / `trust_upgrade` events.
- **Context debt regression.** A workspace that has accumulated a year of memories and summaries produces a debt score within expected bounds; sudden jumps trip warnings.

## Decision gate

Phase 3 passes when:

1. A context build for the alpha demo runs end-to-end. The `~/.ssh/config` block path is visible in the Context Inspector with reason `sensitivity` or `visibility`.
2. The memory candidate from §11 of the alpha demo lands correctly; provenance shows source events; restrict / revoke / delete all function.
3. `delete_memory` zeroes `memory.text` and removes the embedding from the vector store.
4. A capsule lineage test produces an inherited block when a derived item would be sent to a forbidden route — even if the derived item's own row wouldn't trigger the block.
5. A trust downgrade in `shell.run` for a specific actor escalates a previously auto-approved effect into approval.

## Risks

| Risk                                | Mitigation                                                                       |
| ----------------------------------- | -------------------------------------------------------------------------------- |
| Scoring quality                     | Ship a small benchmark task set; track recall@k over time. Pluggable scorer trait so workspaces can swap. |
| PII redaction false positives       | Redaction is opt-in per workspace; default-off for personal mode. Studio shows redacted/unredacted diff for trusted operators. |
| Capsule "policy creep"              | Strictest-wins can over-restrict legitimate work. Mitigated by `weaken_capsule` admin command (audited) and Studio-visible capsule lineage. |
| Embedding model lock-in             | `embedding_ref.embedding_model` recorded; reembed jobs swap model atomically per memory. |
| Trust score gaming                  | Trust is advisory only — never adds authority, only escalates Guard scrutiny. Manual `pin_trust` overrides leave audit events. |

## CLI deliverables (Phase 3)

Per `/planning/cli-design.md` § "CLI staging across phases":

- New subcommands: `actant memory restrict|expire|revoke|delete`, `actant context list|show|inspect`, `actant capsule list|create|attach|retire`, `actant trust show`.
- New templates: `support-agent`, `research-agent`.
- New examples: `memory-review` (full lifecycle), `context-firewall`, `swoosh-scout`.
- `actant studio context inspect ctx_<id> --open` jump from CLI to dashboard.

## Work packages

- `/agents/actant-embed.md`
- `/agents/actant-capsule.md`
- `/agents/actant-trust.md`
- `/agents/phase-3-extensions.md` (extends `actant-context`, `actant-memory`, `actant-policy`, `actant-command`, `actant-storage`, `actant-subscribe`, `actant-server`, plus `actant-cli` for the new subcommands)

## Sequencing

```
week 1
  ├── actant-embed (LanceDB adapter)
  ├── actant-storage row mappers for Phase 3 tables
  └── actant-capsule trait + strictest-wins composer

week 2
  ├── actant-context: gather + score (using actant-embed)
  ├── actant-context: firewall (capsule-aware)
  └── actant-context: redact + truncate + emit

week 3
  ├── actant-memory: lifecycle commands + embedding cascade
  ├── actant-memory: conflict detection
  └── actant-trust: initial computation + recalibration

week 4
  ├── actant-policy reads trust + capsule
  ├── actant-command new commands
  ├── Studio Phase 3 screens
  └── Decision gate
```
