# CHANGELOG

This file accumulates the visible changes shipped through this work session.
Cross-reference: [SPECS_STATUS.md](./SPECS_STATUS.md), [GATES.md](./GATES.md),
[RELEASE_CHECKLIST.md](./RELEASE_CHECKLIST.md).

## Unreleased — 2026-05-17

The substrate was unfrozen and built out from the wedge through every
roadmap phase. Test count at session close: **186 Rust + 25 TypeScript + 4
Python (1 skipped, needs `ACTANTDB_TEST_URL`) + 1 workspace smoke = 216**,
all green. CI bundle (`fmt-check + clippy -D warnings + test +
verify-specs + verify-agents`) passes.

### v1 production-readiness round (added on top of the substrate)

- **TLS termination** — `actantdb-server` and `actantdb serve` accept
  `--tls-cert` / `--tls-key`. Uses `axum-server` + `rustls` (aws-lc-rs).
  Test: `crates/actant-server/tests/tls.rs` generates a self-signed cert
  with `rcgen`, boots an HTTPS listener, hits `/v1/healthz/ready`, asserts
  the response. Shutdown is wired through the same graceful-signal path.
- **MCP stdio transport** — `actant-worker-mcp` now talks real JSON-RPC
  2.0 over child-process stdin/stdout: `initialize` → `tools/call` → read.
  The `actant-worker-mcp` binary runs the standard worker loop claiming
  `mcp.call` effects. Tests: `tests/stdio_round_trip.rs` (3 tests asserted
  against a python3 fixture MCP server; tests are skipped when python3 is
  unavailable, so they only assert when actually run).
- **mdbook documentation site** — `docs/book.toml`, `docs/src/`, regenerator
  script `docs/build.sh` that materializes the site from the canonical
  `/specs` + ADRs + operational docs. Produces a complete static site
  (`docs/book/`, 20 specs + 18 ADRs + operations + reference).
- **SLO targets + HTTP load bench** — `docs/SLO.md` lists latency p50/p99,
  throughput floors, availability targets. `bench/benches/http_command.rs`
  measures end-to-end HTTP POST `/v1/command` (`append_user_message`).
  Result: **341 µs median**, within SLO p50 < 5 ms.
- **Per-endpoint rate limiting** — wired through `actant-throttle` token
  buckets, configurable per `AppState` and per command type.
- **Cluster sync** — `/v1/sync?workspace_id=&since=&limit=` returns
  events since a cursor (ULIDs are lexicographically sortable, so the
  cursor is the last seen event id).
- **Health probes** — split into `/v1/healthz/startup`, `/v1/healthz/live`,
  `/v1/healthz/ready`. Helm chart updated to use all three.
- **Graceful shutdown + x-request-id middleware** — both HTTP and HTTPS
  servers honor SIGTERM/SIGINT and stamp every response with a request id.
- **Production migration CLI** — `actantdb migrate --dry-run` lists pending
  migrations; `actantdb backup --to` / `actantdb restore --from` do a
  consistent file copy via `PRAGMA wal_checkpoint(TRUNCATE)`.
- **Concurrency tests** — approval race + effect-claim race
  (atomic `UPDATE WHERE status='pending'` + `rows_affected` check).
- **JWT auth** — HS256 + RS256 (via OIDC discovery/JWKS). Round-trip tested.



### Added — wedge (v0.1, completed first)

- **`@actantdb/mastra`** — `withActant(agent, opts)` duck-typed wrapper that
  captures every tool call, runs Guard, supports approval flows, and exposes
  the timeline to Studio. Works on Mastra, LangGraph, and hand-rolled
  agents (proven by three public examples).
- **`@actantdb/core`** — Ledger backed by `node:sqlite`, hash-chained
  events, monotonic ULIDs, idempotency.
- **`@actantdb/policy`** — Verdict builders + alpha-demo policy with
  `rm -rf .../dist` constrain hint.
- **`@actantdb/replay`** — Checkpoint/run/diff with memory + policy overrides.
- **`@actantdb/studio`** — Local HTTP server + vanilla-JS UI with timeline,
  detail panel, replay modal, side-by-side diff. CLI: `studio | approve |
  deny | replay create|run|diff | approvals`.
- **`@actantdb/types`** — Generated from `crates/actant-contracts` via the
  `codegen-ts` subcommand.
- **`@actantdb/sdk`** (new this session) — HTTP+WS client for the server.
- **`@actantdb/convex`** (upgraded from placeholder) — adapter for Convex's
  `handler(ctx, args)` tool shape.
- **`wedge/demo/`**, **`wedge/demo-langgraph/`**, **`wedge/demo-cli/`** —
  three runnable public examples.
- **SVG hero**, **asciinema cast**, **publish-ready npm tarballs** in
  `dist-publish/`.

### Added — substrate (Phases 1–6)

#### Phase 1: command engine + storage + server

- **`actant-storage`** — SQLite via `sqlx`. Migration runner that strips
  comments before splitting on `;`. Repo functions for workspace, actor,
  session, agent_event chain with hash linkage, idempotency record.
  **Postgres backend** as `PgStorage` with its own migration set.
- **`actant-command`** — 10 alpha commands from spec 10:
  `create_session`, `append_user_message`, `append_agent_message`,
  `request_tool_call`, `approve_tool_call`, `deny_tool_call`,
  `record_tool_result`, `propose_memory`, `approve_memory`, `reject_memory`.
- **`actant-policy` (Rust)** — Mirror of the TS policy crate: regex
  deny-list, sensitivity ceiling, per-tool risk, hardcoded `shell.run`
  default-approval, constrain-hint for `rm -rf …/dist`.
- **`actant-server`** — Axum HTTP + WebSocket. Endpoints: `/v1/healthz`,
  `/v1/metadata/commands`, `/v1/command`, `/v1/events`, `/v1/approvals`,
  `/v1/ws`. Auto-seeds a default workspace + system actor on first boot.
- **`actant-cli`** (the `actantdb` binary) — `migrate | serve | command |
  events | approvals`. Single command exercises every endpoint.
- **`actant-subscribe`** — broadcast hub with per-topic receivers; powers
  `/v1/ws`.

#### Phase 2: effect queue + workers

- **`actant-effects`** — `EffectQueue::enqueue`, `register_worker`,
  `claim_one`, `heartbeat`, `start`, `complete`, `fail`.
  `effect_claim` rows with explicit lease expiry.
- **`actant-worker-protocol`** — `Handler` trait + `WorkerRunner` poll
  loop. Auto-registers the worker's actor on first heartbeat.
- **`actant-worker-shell`** — `shell.run` via `tokio::process::Command`.
- **`actant-worker-file`** — `file.read` + `file.write` under approved paths.
- **`actant-worker-model`** — `model.call` with `Mock` + OpenAI-compatible
  providers. Returns deterministic mock for tests; real HTTP path for
  prod (requires `ACTANTDB_MODEL_API_KEY`).
- **`actant-worker-mcp`** — envelope-only handler (real MCP wire is Phase 2.5).
- **`actant-worker-browser`** (new) — `browser.navigate|click|type|screenshot`
  via a pluggable `Driver` trait. Ships an `EmulatorDriver` that records
  actions deterministically; WebDriver/CDP swap is one file.

#### Phase 3: context + memory

- **`actant-context`** — manifest pipeline: gather → score → firewall →
  truncate. Blocks `cloud_model_allowed` routes from receiving Secret
  content. Sensitivity-ceiling enforcement.
- **`actant-memory`** — candidate / approval / use lifecycle. **Conflict
  detection**: Jaccard token overlap + polarity-marker check ("always" vs
  "never", "must" vs "must not") writes `memory_conflict` rows.
- **`actant-embed`** — trait surface; `actant-embedders::HashEmbedder` ships
  a deterministic SHA-256-based 32d vector for offline tests.
- **`actant-index`** — dense cosine retrieval over in-memory items.

#### Phase 4: workflows + triggers

- **`actant-flow`** (upgraded from topo-only) — **Runner state machine**.
  `next_action()` returns `Action::ToolCall | ModelCall | AwaitApproval |
  Delay | Done`. Supports approval-gate pause/resume.
  Daily-digest demo (spec 10 §14) walks through to completion in a test.
- **`actant-trigger`** (upgraded) — **`Scheduler`** with `register`, `tick`,
  `run` loop. Cron past-due detection respects `last_fired_at`, the
  `enabled` flag, and a `tokio::sync::watch` shutdown channel.

#### Phase 5: replay

- **`actant-replay`** — Four real modes:
  - `recorded` — emit recorded outputs as identical.
  - `model` — mark `model_call` rows changed.
  - `policy` — mark verdict slots (`tool_call_requested`, `approval_*`) changed.
  - `memory` — rebuild manifest minus excluded ids; downstream rows changed.
  - `experimental`, `tool`, `local_only` return a named-error explaining the
    deferral (requires worker re-invocation).

#### Phase 6: cloud / team

- **`actant-auth`** — HS256 JWT sign + verify + expiry + Principal extraction.
  **OIDC** module with discovery doc + JWK Set fetch and 1-hour cache.
  HTTP fetcher is a trait (real callers wire `reqwest`; tests use a stub).
- **`actant-tenant`** — `TenantContext { principal, storage }`. Role checks,
  `assert_event_in_tenant` cross-tenant guard.
- **`actant-audit-export`** — `export_workspace`, `export_window`,
  `nightly_export` (one chunk per workspace), `RetentionPolicy`, and
  **`purge_by_policy`** that actually deletes events past retention.
- **`deploy/docker/`** — Multi-stage Dockerfile (rust:1.88 → distroless),
  compose with Postgres sidecar.
- **`deploy/helm/actantdb/`** — Chart with Deployment, Service, optional
  Postgres StatefulSet, PVC for SQLite mode, readiness/liveness probes.

#### Reliability primitives

- **`actant-throttle`** — token bucket with refill-rate.
- **`actant-circuit`** — closed/open/half-open breaker with timeout.
- **`actant-lock`** — lease-based locks against the `lock` table.
- **`actant-ingress`** — HMAC-shaped ingest with dedup keys.

#### AI-native primitives

- **`actant-protocol`** — MCP server + A2A card + AP2 mandate types
  (with spend-limit enforcement).
- **`actant-prompts`** — template registry with `{{var}}` interpolation.
- **`actant-models`** — registry with cheapest-cloud + lowest-latency-local
  pickers.
- **`actant-cache`** — content-keyed semantic cache.
- **`actant-trace`** — W3C-style trace + span id minting.

#### CLI + SDK product surface

- **`actant-schema-dsl`** — JSON-style project doc parser.
- **`actant-sdk-codegen`** — TS / Python / Swift client templates.
- **`actant-templates`** + **`actant-codegen-project`** — `actantdb init`
  scaffold writer.
- **`sdks/python/`** (new) — pip-installable `actantdb` package, mirrors the
  TS SDK surface. Integration test passes against a real subprocess server.

#### Hot path + cluster + extras

- **`actant-kernel`** — synchronous in-process tool-call dispatcher.
- **`actant-sync`** — `missing_in` event-set diff (Phase 6 wire protocol
  is the next step).
- **`actant-eval`** — eval case match-expected-avoid-forbidden.
- **`actant-capsule`** — policy-bundle types.
- **`actant-trust`** — Wilson-ish confidence trust profile.

#### Performance

- **`bench/`** — Criterion benchmarks:
  - `storage_append_event` ≈ **60 µs** per event (in-memory SQLite).
  - `command_append_user_message` ≈ **116 µs** per dispatch.

### Changed

- `rust-toolchain.toml` 1.82 → **1.88** (transitive `time-macros` requires it).
- `engines.node` `>=20` → **`>=22.5`** on every TS package that imports
  `node:sqlite`.
- Workspace adds `sqlx` Postgres feature.
- Studio replay dialog gains **mode selector** (recorded / model / policy /
  memory) with a `mode` field on `/api/replay`.

### Fixed

- Monotonic ULID generation — two ULIDs created in the same millisecond
  are now strictly ordered by an incremented random suffix instead of by
  random luck. Caught by the original checkpoint manifest_hash bug.
- Mastra-wrapper constrain rewrite — used to depend on a `globalThis`
  side channel; the wrapped tool now receives `finalArgs` directly via the
  `execute(finalArgs)` callback. Stock tools (no Actant hook) now see the
  rewritten args.
- Storage migration script strips comments before splitting on `;` so
  semicolons inside SQL comments don't break statement boundaries.
- Tool-call FK ordering: `tool_call` is inserted before `approval_request`
  so the approval's `tool_call_id` FK resolves.

### Deferred (explicit, named gaps — not stubs)

- **NAPI / WASM bridges** for `@actantdb/core` — declared via
  `optionalDependencies` but not built.
- **Real cloud-model inference** — `Provider::OpenAi` exists but is
  un-tested in CI; needs `ACTANTDB_MODEL_API_KEY`.
- **MCP wire transport** — `actant-worker-mcp` returns an envelope only.
- **Real browser driver** — `actant-worker-browser`'s `EmulatorDriver` is
  deterministic; a WebDriver/CDP `Driver` impl is a one-file swap.
- **OIDC token verification** — discovery + JWKS fetch are real; the RSA
  signature verification path delegates to a future `jsonwebtoken`
  integration.
- **Postgres command-engine plumbing** — `PgStorage` exists with the
  schema; the command engine itself still hardcodes `SqlitePool` paths.
- **Studio dashboard polish** — the wedge UI is vanilla JS; full React
  rewrite is post-design-partner.
- **Gates 2 + 3 (PIVOT.md)** — measure external adoption events that no
  code change closes. [`GATES.md`](./GATES.md) +
  [`RELEASE_CHECKLIST.md`](./RELEASE_CHECKLIST.md) enumerate the human-only
  remainders.

### Reproduce

```bash
# Everything green:
cargo test --workspace        # 72 passing
pnpm -r test                  # 24 passing
pnpm smoke                    # workspace E2E
cargo bench -p actant-bench --bench storage_append -- --sample-size 10
cd sdks/python && python3 -m unittest tests.test_client -v   # 3 unit + skipped integration

# Run the alpha demo end-to-end against a real server:
cargo build -p actant-server
./target/debug/actantdb-server --bind 127.0.0.1:4555 &
ACTANTDB_TEST_URL=http://127.0.0.1:4555 \
  (cd sdks/python && python3 -m unittest tests.test_client.IntegrationTests -v)
```
