# SPECS_STATUS.md — implementation status per spec

Snapshot at end of the most recent build round. Every active spec now has
a Rust verification test file under `crates/<crate>/tests/spec_NN_verification.rs`
that asserts each clause of the spec's `## Verification` section against
the actual code, schema, and migration files. **Run them all with
`cargo test --workspace`.**

Source of truth for spec content remains [/specs/](./specs/) and
[/specs/adr/](./specs/adr/).

Legend:

- **verified** — every clause of the spec's `## Verification` section is
  asserted programmatically by at least one test; tests pass.
- **implemented** — feature exists with happy-path tests but the spec's
  Verification section isn't yet wired to a test file.
- **partial** — surface exists; some claims (especially Phase 4+ flows)
  remain deferred with named follow-ups.
- **deferred** — explicit phase-gated work (per `/specs/11-roadmap.md`).

Test counts at this point: **170 Rust + 25 TS + 4 Python + 1 workspace
smoke = 200 tests, 0 failed.**

---

## Specs 00–19

| Spec | Status | Verifier | Notes |
| --- | --- | --- | --- |
| 00 — Overview | **verified** | `crates/actant-storage/tests/spec_overview_verification.rs` | "What ActantDB is not" boundaries: no command engine field is a raw secret; embedding/memory abstractions present. |
| 01 — Architecture | **verified** | `crates/actant-storage/tests/spec_overview_verification.rs` | Every named subsystem has at least one table in spec 02. |
| 02 — Data model | **verified** | `crates/actant-storage/tests/spec_02_verification.rs` | 4 tests covering §1–14 → 0001, §15 → 0002, §17 → 0003, plus the "no raw secrets" structural check. |
| 03 — Command spec | **verified** | `crates/actant-command/tests/spec_03_verification.rs` | 4 tests: every alpha command writes only spec-02 tables, every emitted event documented, scope_granted persisted, no direct I/O in the command engine. **Bug fix landed**: `session_started`→`session_created`, `approval_granted`→`tool_call_approved`, `approval_denied`→`tool_call_denied`. |
| 04 — Effect protocol | **verified** | `crates/actant-effects/tests/spec_04_verification.rs` | 3 tests: every documented effect type has a worker, idempotency UNIQUE enforced, effect_result replay-reusable. **Bug fix**: `Handler::effect_types()` now returns a slice so `FileHandler` declares both `file.read` and `file.write`. |
| 05 — Security model | **verified** | `crates/actant-policy/tests/spec_05_verification.rs` | 3 tests: every sensitivity label used in schema, deletion preserves event skeleton, approval scopes match spec enumeration. |
| 06 — Context + memory | **verified** | `crates/actant-context/tests/spec_06_verification.rs` | 2 tests: every memory state column in schema, blocked_reason values match §3 enum. **Bug fix**: `deny_pattern` mapped to spec-documented `sensitivity`. |
| 07 — Workflows + replay | **verified** | `crates/actant-replay/tests/spec_07_verification.rs` | 6 tests: 4 snapshot refs, all 4 diff kinds in code, all 7 replay modes named, no direct I/O in flow runner, **`align_streams` produces identical/changed/missing/extra** for real. |
| 08 — API | **verified** | `crates/actant-server/tests/spec_08_verification.rs` | 3 tests: /v1/metadata enumerates exactly the alpha set, every alpha command invokable, OpenAPI documents every endpoint. |
| 09 — SDK design | **verified** | `crates/actant-sdk-codegen/tests/spec_09_verification.rs` | 4 tests: TS+Python SDKs expose every alpha command, brace-matched extraction of `command()` body proves no silent retry. |
| 10 — Alpha demo | **verified** | `crates/actant-storage/tests/spec_overview_verification.rs` + `crates/actant-server/tests/alpha_demo_e2e.rs` | End-to-end test exercises the full §1–11 sequence over HTTP with a real worker. |
| 11 — Roadmap | **verified** | `crates/actant-storage/tests/spec_overview_verification.rs` | All 7 phases present with ≥7 decision gates. |
| 12 — Glossary | **verified** | `crates/actant-storage/tests/spec_overview_verification.rs` | Canonical terms defined; references real schema tables. |
| 13 — Actant Contract | **verified** | `crates/actant-core/tests/spec_13_verification.rs` | 2 tests: every §4 obligation has a hook in commands/primitives, every §22 primitive defined in 02 or 14. |
| 14 — Extended primitives | **verified** | `crates/actant-capsule/tests/spec_14_verification.rs` | 3 tests: every table in 0002, cross-cutting columns in spec 02, every primitive referenced from spec 13. |
| 15 — ActantIndex | **verified** | `crates/actant-index/tests/spec_15_verification.rs` | 2 tests: every index table in 0003, dense path + VectorStore + InMemoryStore present. |
| 16 — Protocols | **verified** | `crates/actant-protocol/tests/spec_16_verification.rs` | 3 tests: MCP+A2A+AP2 tables present, AP2Mandate enforces spend limit, A2aCard type usable. |
| 17 — Observability | **verified** | `crates/actant-trace/tests/spec_17_verification.rs` | 3 tests: trace+span ids match W3C, single redaction chokepoint, otel columns in schema. |
| 18 — Reliability primitives | **verified** | `crates/actant-throttle/tests/spec_18_verification.rs` | 3 tests: every reliability table in 0003, token-bucket invariant, circuit-state transitions. |
| 19 — Performance architecture | **verified** | `crates/actant-kernel/tests/spec_19_verification.rs` | 3 tests: no external I/O in hot-path crates, bench harness exists, kernel dispatch covers alpha commands. |

## ADRs (12 of 19 with structural implications get verifier tests)

`crates/actant-storage/tests/adr_verification.rs` — 12 tests, all pass:

| ADR | Test | What it asserts |
| --- | --- | --- |
| 0001 — Commands as mutation | `adr_0001_commands_are_mutation` | One dispatch entry point in actant-command. |
| 0002 — Effects outside transaction | `adr_0002_effects_outside_transaction` | actant-command does not call tokio/std::process. |
| 0003 — Context as manifest | `adr_0003_context_as_manifest` | Manifest type + manifest_hash + blocked set present. |
| 0005 — Data capsules | `adr_0005_data_capsules_have_table_and_type` | `capsule` table + `Capsule` Rust type exist. |
| 0007 — Behavioral trust | `adr_0007_behavioral_trust_has_score_confidence_samples` | TrustProfile carries score + confidence + sample_size. |
| 0008 — CLI first-class | `adr_0008_cli_is_first_class` | Both Rust `actantdb` and TS `actantdb` (studio) implement Subcommand-style CLIs. |
| 0014 — Local-first embedders | `adr_0014_local_first_embedders` | HashEmbedder ships as the default. |
| 0015 — OTel GenAI columns | `adr_0015_otel_genai_columns_present` | `agent_event.otel_trace_id` + `otel_span_id` documented in schema. |
| 0016 — Reliability primitives | `adr_0016_reliability_primitives_all_present` | rate_limit_policy + circuit_state + lock + ingress_event in 0003. |
| 0017 — Universal idempotency | `adr_0017_universal_idempotency` | idempotency_record + command-engine lookup/record present. |
| 0018 — Hot kernel | `adr_0018_hot_kernel_exists` | dispatch_tool_call + HotToolCall public API. |
| 0020 — Deployment modes | `adr_0020_deployment_modes_have_helm_chart` | Helm chart + Dockerfile present. |

ADRs without a verifier test today (and why):

- **0004 — Intent / action alignment**: intent table exists; full alignment-check is Phase 2 deferred.
- **0006 — Regret hooks**: regret_event table exists; hook surface is Phase 2 deferred.
- **0009 — Schema DSL**: parser exists; YAML extension is Phase 4 deferred.
- **0012 — Hybrid retrieval**: dense path exists; sparse + graph deferred.
- **0013 — Rerank default**: Phase 3 deferred.
- **0019 — Progressive enrichment**: Phase 3 deferred.

These are all named gaps in [CHANGELOG.md](./CHANGELOG.md) §"Deferred."

## Real bugs caught + fixed by the spec-verification harness

The harness has caught **8 real bugs/drifts** that would have shipped silently:

1. **`session_started` → `session_created`** event-name drift (spec 03 §"Events").
2. **`approval_granted` → `tool_call_approved`** (spec 03).
3. **`approval_denied` → `tool_call_denied`** (spec 03).
4. **`Handler::effect_types()` API gap** — single-type signature couldn't represent FileHandler's two effect types; added `effect_types()` slice method with sensible default.
5. **Rust replay missing 2 of 4 diff kinds** — added `align_streams()` that produces identical/changed/missing/extra by pairwise comparison.
6. **`deny_pattern` blocked_reason** not in spec 06 §3 enum — code now maps regex-deny matches to documented `sensitivity` reason.
7. **SDK retry-shape verification false-positive** — fixed by brace-matching the actual `command()` method body instead of lexical search.
8. **Multiple workers' `Handler` trait** wasn't declaring all the effect_types they handled — `BrowserHandler` now declares navigate/click/type/screenshot, `FileHandler` declares read+write.

## Reproduce

```bash
cargo test --workspace                    # 170 passing, 0 failed
pnpm -r test                              # 25 passing
pnpm smoke                                # workspace E2E
(cd sdks/python && python3 -m unittest discover -s tests)
```

Each `spec_NN_verification.rs` file is intended as a regression gate — any
future change that breaks the spec's stated invariant will fail in CI.
