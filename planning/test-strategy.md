# Test strategy

Cross-phase. Every crate's work package names its unit + property + integration tests. This document specifies the *categories*, the *tooling*, the *coverage targets*, and the *invariants* that test failures must surface.

## Pyramid

```
                    ┌─────────────────┐
                    │  end-to-end     │ runs the alpha demo + workflow demo
                    ├─────────────────┤
                    │  integration    │ multi-crate, with sqlx + axum running
                    ├─────────────────┤
                    │  property-based │ proptest / quickcheck for invariants
                    ├─────────────────┤
                    │  unit tests     │ single-function correctness
                    └─────────────────┘
```

## Per-category targets

| Category        | Tool              | Target                                       | Where                              |
| --------------- | ----------------- | -------------------------------------------- | ---------------------------------- |
| Unit            | `cargo test`      | Per-function correctness                     | `crates/*/src/`                    |
| Property        | `proptest`        | Invariants from `/specs/05-security-model.md` | `crates/*/tests/`                  |
| Integration     | `cargo test`      | Cross-crate flows w/ ephemeral SQLite         | `crates/*/tests/`                  |
| End-to-end      | `cargo test -p e2e` | Server + workers + Studio API                | `tests/e2e/` (new in Phase 2)      |
| Workflow durability | `cargo test`  | Process-restart survival                      | `crates/actant-flow/tests/`        |
| Replay determinism | `cargo test`   | Recorded-replay diff = identical              | `crates/actant-replay/tests/`      |
| Worker conformance | `cargo test`  | Protocol contract                             | `crates/actant-effects/tests/`     |
| Studio          | Vitest + Playwright | Routes, WCAG, perf                          | `studio/tests/`                    |
| SDK             | per-language       | Generated surface + transport                | `sdks/*/tests/`                    |

## Invariant tests

Every invariant in `/specs/05-security-model.md` §2 maps to at least one test.

| Invariant                                                       | Test                                                              | Crate              |
| --------------------------------------------------------------- | ----------------------------------------------------------------- | ------------------ |
| 1. No mutation without a command                                | grep test: no `INSERT INTO` outside `actant-storage`              | architectural test |
| 2. No command without an actor                                  | DB schema check + dispatcher test                                  | `actant-command`   |
| 3. No sensitive command without authority                       | property test: random scope sets + commands                       | `actant-policy`    |
| 4. No side effect inside a database transaction                 | grep test: no HTTP / process spawn inside a `Transaction` block   | architectural test |
| 5. No model call without a context manifest                     | command test: `request_model_call` without `context_build_id` rejected | `actant-command`   |
| 6. No memory without provenance                                 | `approve_memory` test: missing source_event_ids rejected           | `actant-memory`    |
| 7. No cloud context without visibility policy                   | context-build firewall property test                              | `actant-context`   |
| 8. No approval without audit record                             | approval flow integration test asserts audit_event row             | `actant-command`   |
| 9. No replay without policy snapshot                            | replay precondition test                                          | `actant-replay`    |
| 10. No secret in ActantDB tables                                | grep test: no `secret_ref` column type contains raw material      | `actant-storage`   |
| 11. No cross-workspace effect                                   | property test: random workspace ids                               | `actant-effects`   |
| 12. No silent overrides                                         | `policy.override` test: emits `audit_event` with reason            | `actant-policy`    |

These tests run in CI as a separate `invariants` job and gate every PR.

## Property test seeds

- Per-PR: deterministic seeds.
- Nightly: random seeds, 10x iteration count, failures filed automatically as issues with seed + reproducer.

## End-to-end (Phase 2+)

`tests/e2e/` runs the alpha demo and the workflow demo from `/specs/10-alpha-demo.md` end-to-end:

1. Start `actantdb-server` in a child process.
2. Start the four reference workers in child processes.
3. Drive the demo via the TypeScript SDK.
4. Assert: every Studio subscription target received the expected sequence of events.
5. Tear down.

## Performance gates

Per `/planning/phase-N-plan.md`:

- Command latency p99 ≤ 50ms for the alpha command set.
- Subscription delivery latency p99 ≤ 200ms.
- Replay reconstruction time ≤ 100ms per event for `mode=recorded`.
- 5k-event Audit Trail render ≤ 1s in Studio.

A `bench/` package measures these and produces a JSON report per PR; regressions > 10% fail CI.

## Determinism harness

`crates/actant-replay/tests/determinism.rs` (Phase 5): snapshot-replay round-trip for `mode=recorded` MUST produce `kind='identical'` for every event in a curated 1000-event session.

## CI matrix

| Matrix axis    | Values                                              |
| -------------- | --------------------------------------------------- |
| OS             | ubuntu-latest, macos-latest                         |
| Rust toolchain | 1.82.0 (pinned in `rust-toolchain.toml`)            |
| SQLite version | latest stable                                       |
| Node           | 20, 22 (for SDK + Studio)                           |
| Python         | 3.10, 3.11, 3.12                                    |

## Coverage

Tarpaulin reports per-crate coverage; CI blocks merging if line coverage drops below 80% in any non-glue crate (`actant-core`, `actant-storage`, `actant-command`, `actant-policy`, `actant-effects`, `actant-context`, `actant-memory`, `actant-flow`, `actant-replay`, `actant-subscribe`).

## What we do NOT test

- LLM model output. Models are external; we test their adapters, not their content.
- Vendor-specific APIs (OpenAI, Anthropic) — we contract-test our adapter against a recorded fixture, not their live service.

## Test data

- Fixture sessions live under `tests/fixtures/sessions/` (Phase 2).
- Synthetic actor histories live under `tests/fixtures/actors/`.
- A "regression session" lives under `tests/fixtures/regression/` and runs as part of every CI build.
