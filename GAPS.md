# GAPS — known implementation gaps

Open items that have planning coverage but are not yet fully implemented.
Cross-reference: [CHANGELOG.md §Deferred](./CHANGELOG.md), [GATES.md](./GATES.md).

Status legend: 🟢 closed this pass · 🟡 deferred (named, not silent) · 🔴 open · 👤 human-only

| # | Gap | Status | Where documented | Notes |
|---|-----|--------|-----------------|-------|
| 1 | **Swift SDK source** | 🟢 | `agents/sdk-swift.md`, `specs/09-sdk-design.md` §10 | `sdks/swift/` scaffolded this pass (alpha-command surface, AsyncSequence subscription stub). |
| 2 | **Rust SDK source** | 🟢 | `agents/sdk-rust.md`, `specs/09-sdk-design.md` §11 | `sdks/rust/` scaffolded this pass (alpha-command surface, separate crate, not a workspace member). |
| 3 | **MCP wire transport** | 🟢 | `agents/actant-worker-mcp.md` | `crates/actant-worker-mcp/tests/stdio_round_trip.rs` covers `initialize` + `tools/call` round-trip + missing-program error. 3 tests pass. |
| 4 | **Real browser driver** | 🟡 | CHANGELOG §Deferred | `EmulatorDriver` is deterministic; a WebDriver/CDP impl is a one-file swap. Deferred until a demo needs it. |
| 5 | **Postgres command-engine** | 🟡 | CHANGELOG §Deferred | `PgStorage` exists with schema; the command engine still hardcodes `SqlitePool` paths. Deferred until a deployment needs it. |
| 6 | **Studio full React rewrite** | 🟡 | CHANGELOG §Deferred | Post-design-partner; current Studio is vanilla JS wedge — works, ships, replays. |
| 7 | **`experimental` / `tool` / `local_only` replay modes** | 🟡 | `specs/07-workflows-and-replay.md`, CHANGELOG §Deferred | Named-error stubs; require replay-scoped worker re-invocation. Phase-5 follow-up. |
| 8 | **Gate 2 + Gate 3** | 👤 | `GATES.md` | Blocked on human outreach — npm publish, developer adoption, design partners. |
| 9 | **90-sec screencast + hero PNG** | 👤 | `GATES.md` §"Gate 1 leftovers" | Human-produced artifacts; not automatable. |
| 10 | **Seed eval JSON files** | 🟡 | `agents/actant-eval.md` | `actant-eval` crate ships the `EvalCase` + `run()` surface; the success-criteria DSL + a populated `evals/seed/` corpus are Phase 4 deferred. The originally-referenced `planning/eval-catalog.md` was removed. |
| 11 | **`examples/` subdirectories** | 🟡 | (originally `examples/README.md`) | The `examples/` skeleton was removed; the Phase 1 demos live at `wedge/demo`, `wedge/demo-langgraph`, `wedge/demo-cli` and serve the same purpose. Re-creating an `examples/` tree is a packaging decision deferred until there's a second-framework adapter to demo. |
| 12 | **`templates/` subdirectories** | 🟡 | `templates/README.md` | Skeleton only; no `templates/<name>/` subdirs exist. `actant-templates` crate emits a bare `package.json` + `README.md`, not the 9 named templates. Deferred — no consumer yet. |

## What "100% complete" means

This repo distinguishes:

- **Closed gaps (🟢)** — code + tests in the repo this pass.
- **Named deferrals (🟡)** — scope explicitly out for this milestone; recorded in `CHANGELOG.md §Deferred` and the spec text. Closing them requires a future PR, not a status change here.
- **Human-only (👤)** — actions that no agent in this repo can take (publish, outreach, record video).

A "100% green" snapshot is artifact-shaped: every 🟢/🟡/👤 item is *known* and *documented*; no silent stubs remain. The 🟡/👤 rows do not block validation gates — they block product evolution past the wedge.
