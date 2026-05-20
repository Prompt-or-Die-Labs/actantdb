# CHANGELOG

This file accumulates the visible changes shipped through this work session.
Cross-reference: [SPECS_STATUS.md](./SPECS_STATUS.md), [GATES.md](./GATES.md),
[RELEASE_CHECKLIST.md](./RELEASE_CHECKLIST.md).

## 0.0.8 — 2026-05-18

- **`CommandEngine::dispatch()`** auto-creates the caller's actor row if
  it doesn't exist. Closes the trial-3 FK trap: every fresh consumer's
  first command (any command, not just `create_session`) used to return
  a cryptic `500: storage error: error returned from database: (code:
  787) FOREIGN KEY constraint failed`. The `command_record` and
  `session` tables both FK `actor_id` → `actor(id)`; pre-fix you had to
  manually insert an actor row before any wire call. Now zero-bootstrap.
- **Rust workspace bumped `0.0.5 → 0.0.7 → 0.0.8`** to track npm. Every
  `[workspace.dependencies]` `actant-*` pin updated in lockstep.
  `actantdb --version` now prints `actantdb 0.0.8` (was stuck at 0.0.5
  on the v0.0.7 release because Cargo wasn't synced).
- **`release-binaries.yml`** sha256 sidecar fix. shasum ran from above
  `dist/`, so the sidecar carried `dist/actantdb-...tar.gz` which broke
  `shasum -c` verification anywhere except the workflow runner. Now
  runs from inside `dist/`; sidecar contains just the filename.
- **`SessionRole`** doc comment clarifies `.assistant` vs `.agent`.
- **`COMPARISON.md`** (473 lines) — competitive landscape vs Temporal,
  Inngest, Restate, DBOS, Mastra, LangGraph, OpenAI Agents, CrewAI,
  Langfuse, LangSmith, Phoenix, Helicone, Mem0, Zep. Three honest
  differentiators (hash-chained ledger, Guard verdict as typed event,
  capsule sensitivity ceiling) + explicit "where competitors win".
- **Multi-scenario E2E suite** added at `/tmp/actantdb-scenarios/`
  (approval flow, replay overrides, 20-concurrent-session burst,
  server-mode via `@actantdb/sdk` against the prebuilt binary).
- All 8 packages bumped to **0.0.8**.

## 0.0.7 — 2026-05-18

- **`@actantdb/core`** `openLedger({ inMemory: true })` opens a
  `:memory:` SQLite ledger — closes filed issue #4. Useful for tests
  and any in-process scenario where you want to share one ledger
  between an agent and Studio without touching disk.
- **CI workflow renamed** `publish-shadow.yml → publish-npm.yml` to
  reflect actual behavior (publishes to `latest`, mirrors to `shadow`).
- **Docs sweep**: README test counts updated to 429 (331 Rust + 25 TS
  + 10 Python + 62 Swift + 1 smoke; previous figure was 216).
  `RELEASE_CHECKLIST.md` rewritten to mark Step 1 (npm publish) done
  and reference the workflow instead of deleted `dist-publish/`.
  `waw.md`, `CLAUDE.md` updated. CHANGELOG fills the 0.0.2–0.0.6 gap.
- All 8 packages bumped to **0.0.7**.

## 0.0.6 — 2026-05-18

- **`@actantdb/studio`** now exposes `startStudioServer` as a library
  export (`main` + `types` + `exports`). Previously the package had no
  `main`/`exports` and library users hit `ERR_PACKAGE_PATH_NOT_EXPORTED`
  on `import('@actantdb/studio')`. CLI `bin` entry unchanged.
- All 8 packages bumped to **0.0.6**.

## 0.0.5 — 2026-05-18

- **DX trial 2 fixes** caught from a fresh-user project trial against 0.0.4:
  - `actantdb` CLI: VERSION constant now read from `package.json` (was
    still hard-coded `"0.0.1-pre"` despite the publish version moving).
  - `actantdb studio` now supports `--quiet` to suppress the listening
    banner.
  - README "Install (the substrate)" snippets switched from `ts` to `js`
    fence labels — they're plain JavaScript; the `ts` tag implied a
    TS toolchain a fresh user shouldn't need.
  - README now documents direct ledger access via
    `wrapped.actant.ledger.query({})`.
  - README `@actantdb/studio` install moved to `--save-dev` (it's a
    dev tool; shouldn't ship in consumer production deps).
- **Rust workspace** version bumped `0.0.1 → 0.0.5` for parity with npm;
  every `[workspace.dependencies]` `actant-*` pin updated.
- **`release-binaries.yml`** workflow added — manual + tag-triggered.
  Builds `actantdb` + `actantdb-server` for macOS-arm64, macOS-x64,
  linux-x64; uploads SHA256 sidecars; creates GitHub Release on tag.
  This is the binary-distribution path the Swift `ActantDBSupervisor`
  consumes.
- 4 deeper DX gaps filed as GH issues #1–#4 (numeric policy DSL,
  replay policy override propagation, per-framework prereqs docs,
  in-memory ledger — #4 already closed in 0.0.6).

## 0.0.4 — 2026-05-18

- **`@actantdb/core`** `VERSION` constant now derived from `package.json`
  via `createRequire` (was hard-coded `"0.0.1-pre"`).
- **`@actantdb/core`** `openLedger` accepts both positional
  `(project, storeDir)` and object `({project, storeDir, dbPath})`
  forms. Avoids the `TypeError [ERR_INVALID_ARG_TYPE]` a fresh user
  hit when copying the object-form snippet from the README.
- **`@actantdb/mastra`** marks `@mastra/core` as optional via
  `peerDependenciesMeta` (the wrapper accepts any tools-record-shaped
  agent — not strictly Mastra). Removes spurious peer warnings.
- **`@actantdb/studio`** server handles `HEAD /` as `GET /` for
  liveness probes (was returning 404).
- **`docs/book/`** rendered output untracked + `.gitignore`d
  (`mdbook build docs` regenerates on demand). 80 stale HTML files
  removed.
- **Publish workflow** default tag changed from `shadow` → `latest`;
  added `also_tag_shadow` input (default `true`) so the shadow channel
  still mirrors every release.
- All 8 packages bumped to **0.0.4**. Default `npm install
  @actantdb/X` now gets the fixed version (`latest` is no longer
  pinned to broken-URL 0.0.2).

## 0.0.3 — 2026-05-18

- **Repo URL fix** across every published manifest, `Cargo.toml`,
  `docs/book.toml`, and `docs/src/README.md`. Was
  `github.com/actantdb/actantdb` (7 pkgs) or `github.com/actant/actant`
  (`@actantdb/mastra`); now `github.com/Prompt-or-Die-Labs/actantdb`.
- All 8 packages bumped to **0.0.3** and published to the `shadow` tag.

## 0.0.2 — 2026-05-18

- **First public publish.** Eight `@actantdb/*` packages on npm under
  the `shadow` dist-tag via the new GH Actions `publish-npm.yml`
  workflow. Node 24 in CI so `node:sqlite` is unflagged.
- `packages/` brought into git (had been silently caught by the
  `Packages/` rule in `.gitignore` on case-insensitive APFS).
- Stale `dist-publish/` tarball directory removed.

## Unreleased — 2026-05-18

### Swift SDK opinionated facade + extra storage endpoints

Built to back the Swoosh consumer (`planning/sdk-swift.md`) with minimum
glue code on the consumer side.

- **New high-level Swift module `ActantAgent`** (`sdks/swift/Sources/ActantAgent/`)
  on top of the existing low-level `ActantDB` SDK. Six surfaces — consumers
  add ActantDB by one-line conformance extensions, not by writing adapters:
  - `AgentBackend` (actor) — holds the `ActantClient`, exposes `waitForReady`.
  - `Session<Message>` — generic over the consumer's message type; round-trips
    transcripts via injected `encode`/`decode` closures.
  - `MemoryStore` — propose / approve / reject + `listApproved / listPending /
    listConflicts`.
  - `Auditor<Record>` — generic over a Codable audit record type; round-trips
    through a JSON sentinel in the session ledger.
  - `ApprovalCenter` — pending list + approve / deny / approve-with-constraint.
  - `ReplayClient` — checkpoint / run / diff wrapper.
  - `RelationshipStore` — `upsertEntity / link / entities / neighbors` over
    the new `/v1/entities` and `/v1/entity-relations` endpoints.
  - `ActantDBSupervisor` (actor) — spawns + lifecycles the `actantdb` Rust
    server subprocess for local-first mode; binary discovery (env override,
    `PATH`, `~/.cargo/bin`, app-bundle search paths), stderr port parsing,
    readiness polling, SIGTERM-then-SIGKILL stop.
- **Ten new server endpoints** on `actant-server`, no schema migrations
  (reuse existing `memory`, `memory_candidate`, `memory_conflict`,
  `authority_scope`, `artifact`, `agent_event`, `entity`, `entity_relation`
  tables):
  - `GET    /v1/memories?workspace_id=&status=approved|pending|rejected|all`
  - `GET    /v1/memories/conflicts?workspace_id=`
  - `GET    /v1/permissions?workspace_id=`
  - `POST   /v1/permissions   { workspace_id, actor_id, permission, level, scope?, allowed_actions? }`
  - `DELETE /v1/permissions   { workspace_id, authority_scope_id }`
  - `POST   /v1/setup-reports { workspace_id, actor_id, content }`
  - `GET    /v1/setup-reports?workspace_id=&latest=true`
  - `POST   /v1/scout-records { workspace_id, actor_id, source_id, kind, sensitivity, content, metadata? }`
  - `GET    /v1/scout-records?workspace_id=&source=`
  - `GET    /v1/entities?workspace_id=&type=` / `POST /v1/entities { workspace_id, type, canonical_name, aliases?, sensitivity?, capsule_id?, source_events? }`
  - `GET    /v1/entity-relations?workspace_id=&entity=&relation_type=` / `POST /v1/entity-relations { workspace_id, source_entity, relation_type, target_entity, confidence?, evidence_events? }`
- **Eleven new methods on the low-level `ActantClient`** mirroring those
  endpoints, plus matching Codable row types in `Sources/ActantDB/Types/Storage.swift`
  (`ApprovedMemory`, `MemoryCandidate`, `MemoryConflict`, `MemoryRow`,
  `AuthorityScopeRow`, `SetupReportRow`, `ScoutRecordRow`, `EntityRow`,
  `EntityRelationRow`).
- **OpenAPI** (`crates/actant-server/openapi.yaml`) extended with the 10 new
  paths.
- **Storage shape deviation surfaced**: setup_reports + scout_records each
  append an `agent_event` (event_type `setup_report` / `scout_record`,
  content in `payload_inline`) AND insert an `artifact` row whose `uri` is
  `actantdb://event/<event_id>`. `artifact` is NOT NULL on `uri` with no
  inline body column, and `context_item` requires a NOT NULL
  `context_build_id` — neither is a natural home for free-form caller
  content without a migration.
- **Tests**:
  - Rust workspace: actant-server grew from 20 → 44 passing.
  - Swift SDK: 25 → 62 passing across 12 suites (10 new ActantDBTests for
    the storage endpoints + entities, 27 new ActantAgentTests for the
    facade modules + supervisor + RelationshipStore).
- **Repo baseline fix**: `crates/actant-tenant/Cargo.toml` was missing a
  `[dev-dependencies] proptest = { workspace = true }` declaration its test
  file expects (introduced in `ed21078`). Added.

## Unreleased — 2026-05-17

The substrate was unfrozen and built out from the substrate through every
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



### Added — v0.1 baseline

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
- **`examples/test-cleanup/`**, **`examples/langgraph-router/`**, **`examples/cli-only/`** —
  three runnable public examples.
- **SVG hero**, **asciinema cast**, and a manual-trigger publish workflow
  (`.github/workflows/publish-shadow.yml`) that builds + tests + publishes
  every `@actantdb/*` package to npm under the `shadow` dist-tag.

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
- **Studio dashboard polish** — the React Studio exists; future work is
  feature depth and panel-level polish, not a validation gate.

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
