# GAPS — known implementation gaps

Open items that have planning coverage but are not yet fully implemented.
Cross-reference: [CHANGELOG.md](./CHANGELOG.md) · [GATES.md](./GATES.md) ·
[STORAGE_AUDIT.md](./STORAGE_AUDIT.md) · [UI_AUTH_DESIGN.md](./UI_AUTH_DESIGN.md) ·
[COMPARISON.md](./COMPARISON.md) · [BENCHMARKS.md](./BENCHMARKS.md) · [TESTING.md](./TESTING.md).

Status legend: 🟢 closed · 🟡 deferred (named, not silent) · 🔴 open / actively wrong · 👤 human-only

Last updated: 2026-05-19. Workspace at **36 crates** (was 53 — slim refactor merged
4 reliability + 5 observability + 8 worker crates into 3 bundles; deleted 3 empty crates).
Packages at `@actantdb/*@0.0.10` + new umbrella `actantdb@0.0.10` (re-exports
everything). Rust umbrella `actantdb` crate ships the same shape on the Rust side.

## Status table

| # | Gap | Status | Where documented | Notes |
|---|-----|--------|-----------------|-------|
| 1  | **Swift SDK source** | 🟢 | `agents/sdk-swift.md`, `specs/09-sdk-design.md` §10 | `sdks/swift/` ships `ActantDB` (low-tier HTTP+WS) + `ActantAgent` (high-tier facade) + `ActantDBSupervisor`. 62 tests in 12 suites pass; verified end-to-end against a downloaded `v0.0.9` binary in [`TESTING.md` §"Scenario 7"](./TESTING.md). |
| 2  | **Rust SDK source** | 🟢 | `agents/sdk-rust.md`, `specs/09-sdk-design.md` §11 | `sdks/rust/` ships `actantdb-client` crate: `src/{lib,client,error}.rs`, workspace member, mirrors the Python+Swift surface. `tests/{client_smoke,errors_typed}.rs`. |
| 3  | **MCP wire transport** | 🟢 | `agents/actant-worker-mcp.md` | `crates/actant-workers/tests/mcp_stdio_round_trip.rs` (post-merge location; was `actant-worker-mcp/`) covers `initialize` + `tools/call` + missing-program error. |
| 4  | **Real browser driver (CDP)** | 🟢 | `crates/actant-workers/src/browser/cdp.rs` (664 LOC), `README.md` | `CdpDriver` against Chromium DevTools Protocol, feature `cdp` (off by default — runner doesn't ship Chrome). `tests/browser_cdp_smoke.rs` runs when `CHROME_PATH` is set. Spawn logic mirrors Swift's `ActantDBSupervisor`. |
| 5  | **Postgres command-engine** | 🟡 | `STORAGE_AUDIT.md`, `crates/actant-storage/src/pg_repo.rs`, `migrations/pg/0001-0004` | `PgStorage::impl Repo` now ships **13 repo methods** (parity with the SQLite path's `Storage::impl Repo`). Schema parity: 56 of 90 tables (62%, was 7 of 87 = 8%). The remaining 34 tables are observability + advanced retrieval surfaces not on the command-engine's hot path; the CI parity gate (row #22) records the gap on every run so it stays visible. Status: down from 🔴 to 🟡 — the engine works end-to-end against PG now, but full schema mirror is Phase-6 follow-up. |
| 6  | **Studio React rewrite** | 🟢 | `packages/actant-studio/ui-src/`, `vite.config.ts`, `vitest.config.ts` | React 19 + Vite 5 rewrite. 5 panels (Runs/Timeline/Approvals/EventDetail/Replay) + `lib/api.ts`. 8 vitest tests. Bundle 204 kB / 64 kB gzipped. Server.ts API unchanged — pure UI swap. Live updates poll (2 s tick); WS upgrade is a follow-up the design doc names. |
| 7  | **Replay modes (`experimental` / `tool` / `local_only`)** | 🟢 | `crates/actant-replay/src/lib.rs` (`tool_diff`, `local_only_diff`, `experimental_diff`), `crates/actant-replay/tests/local_only_mode.rs`, `packages/actant-replay/src/index.ts` | All seven modes (recorded / model / policy / memory / tool / local_only / experimental) ship and are tested on both Rust and TS. Rust: 6 unit tests in `lib.rs` + integration tests; experimental mode now produces a `would_reinvoke:<event>` diff with named summary instead of an opaque `NotImplemented` error; local_only mode now annotates `changed` model_call rows with `would_route_local_only` summaries. TS: 9 vitest tests in `@actantdb/replay/src/index.test.ts`. |
| 8  | **Gate 2 → npm publish** | 🟢 | `GATES.md` §"Gate 2" §"What humans must do" | All 8 `@actantdb/*` packages + new `actantdb` umbrella published to npm under `latest` + `shadow`. Currently at `0.0.10`. CI: [`.github/workflows/publish-npm.yml`](./.github/workflows/publish-npm.yml). |
| 9  | **Gate 2 → developer-outreach + Gate 3 → design-partner** | 👤 | `GATES.md` §§"Gate 2", "Gate 3" | Blocked on **human outreach**. Every artifact prerequisite is in place; the metric ("10 devs tried it / 2 design partners signed") only closes via actions outside the repo. Not a code-shaped gap. |
| 10 | **90-sec screencast + hero PNG** | 👤 | `GATES.md` §"Gate 1 leftovers", `docs/SCREENCAST_SCRIPT.md` | **Human-produced** (camera + microphone). Storyboard + 90-second cue-by-cue script ship in `docs/SCREENCAST_SCRIPT.md` (timestamp, what to type, what to point at, expected on-screen result). Recording is the last mile; not a code-shaped gap. |
| 11 | **Seed eval JSON corpus** | 🟢 | `evals/seed/`, `crates/actant-eval/tests/seed_corpus_loads.rs` | 8 seed cases: rm-dist-constrained, refund-over-100, test-prefix-blocked, chain-integrity, cost-under-budget, latency-p99, no-secret-leak, replay-deterministic. Schema-lock test asserts every case parses + has a non-empty `success_criteria.all_of`. |
| 12 | **`examples/` subdirectories** | 🟢 | `examples/README.md` | `examples/{test-cleanup,langgraph-router,cli-only}` symlink the three real demos under `examples/test-cleanup*/`. One source of truth, two paths discoverable. |
| 13 | **`templates/` subdirectories** | 🟢 | `templates/{minimal,coding-agent,research-agent,support-agent,fanout-agent}/`, `crates/actant-templates/src/registry.rs` | 5 templates: `minimal`, `coding-agent`, plus new `research-agent` (web.search + web.fetch), `support-agent` (require_approval + custom resolver), `fanout-agent` (N concurrent sessions). Registry recognizes each. |
| 14 | **Object storage abstraction (S3 / GCS / Azure / IPFS / FS)** | 🟢 | `crates/actant-objectstore/` | `BlobStore` trait + `FilesystemStore` + `MemoryStore` + `S3Store` (feature `s3`, includes presign) + `IpfsStore` (feature `ipfs`) + `Layered`. `Storage::put_artifact(...)` writes through it. 15 + 4 new tests. |
| 15 | **`ACTANTDB_DATABASE_URL` silent-ignore** | 🟢 | `crates/actant-server/src/bin/server.rs` | Server refuses to start if `ACTANTDB_DATABASE_URL` is set while the PG path is still incomplete (with password redacted in error). Helm `storage.backend=postgres` surfaces immediately instead of silently downgrading to SQLite. |
| 16 | **`actant-sync` advertised destinations vs reality** | 🟢 | `crates/actant-sync/src/destinations/{fs,s3,gcs,azure,ipfs}.rs`, `runner.rs`, `destination.rs` | Real `Destination` trait + 5 implementations (FilesystemDestination always on; S3/GCS/Azure/IPFS feature-gated through `actant-objectstore`). `SyncRunner` pulls events from `actant_storage::Storage` since cursor and pushes batched. 7 tests (`filesystem_destination_roundtrip`, `idempotency`, `cursor_resume`, + feature-gated S3/GCS/Azure/IPFS smoke). README rewritten. |
| 17 | **Linking-code UI auth flow for non-loopback bind** | 🟢 | `UI_AUTH_DESIGN.md`, `crates/actant-auth/src/{link,password,session}.rs`, `crates/actant-server/src/auth_routes.rs` | Loopback bind = trust OS user. Non-loopback = refuse without TLS or `--insecure-public`; print one-time `xxxx-xxxx-xxxx` code (60-bit entropy, 15-min TTL, sha256-stored). `/v1/auth/{link,password,login,logout,me}` + argon2id + HttpOnly + Secure + SameSite=Lax cookie + `X-CSRF-Token` on writes. Migration `0004_auth.sql`. 35 tests across actant-auth + actant-server. |
| 18 | **Studio browser auto-open in local mode** | 🟢 | `packages/actant-studio/src/cli.ts` | `actantdb studio` auto-opens default browser on loopback URLs. `--no-open` / `ACTANTDB_NO_OPEN=1` opt-outs. Spawn errors swallowed. |
| 19 | **WASM-reducer parity with SpacetimeDB v2** | 🟢 | `specs/00-overview.md` §"Inspiration, not parity" | **Closed as explicit non-goal.** ActantDB's typed Rust command engine + Guard verdicts is the agent-shaped equivalent. Status is 🟢 because there is no engineering work item here — the absence is the design, and the design is documented. Re-evaluate only if a consumer asks for user-supplied WASM modules running inside the DB process. |
| 20 | **Row-level subscription predicates** | 🟢 | `crates/actant-subscribe/src/predicate.rs`, `crates/actant-subscribe/tests/{predicate_eval,subscribe_filters_messages}.rs` | `Predicate` enum: `Field` + `Literal` leaves, `Eq/Ne/Lt/Le/Gt/Ge` comparators, `And/Or/Not` logic, `Exists`. `SubscribeHub::subscribe(topic, predicate)` filters at fanout. Two test files cover all comparators + multi-subscriber filtering. |
| 21 | **Point-in-time recovery / incremental backup** | 🟢 | `crates/actant-storage/src/backup.rs`, `crates/actant-cli/src/main.rs` (`Backup { mode }` + `Restore { from, at_lsn }`) | Library helpers (`Storage::last_lsn`, `wal_frames_since`, `apply_wal_frames`, `Manifest`) landed earlier; CLI wiring landed in this pass. `actantdb backup --to <file> --mode=full` does a `wal_checkpoint(TRUNCATE)` + file copy; `actantdb backup --to <dir> --mode=incremental` writes a full snapshot on first call + a WAL increment on each subsequent call + manages `manifest.json`. `actantdb restore --from <file>` accepts either a single full snapshot OR a directory (auto-detected); `--at-lsn N` stops replay at that LSN. |
| 22 | **Postgres schema parity CI gate** | 🟢 | `.github/workflows/ci.yml` `migrations-parity` job | New `migrations-parity` job extracts `CREATE TABLE` names from `migrations/*.sql` and `migrations/pg/*.sql`, asserts no Postgres-only table exists (hard fail), reports the SQLite-only diff as a `::notice::` on every run so the lag is visible. Will tighten to hard-fail when row #5's catching-up is done. |
| 23 | **Empty `sdks/rust/` target dir** | 🟢 | row #2 closure | Resolved together with #2 — `sdks/rust/` is now a real crate (`actantdb-client`) and a workspace member. The stale `target/` artifact is gone (and target now lives in `~/.cache/cargo-actantdb` per `.cargo/config.toml`). |

## What "100% complete" means

- **🟢 Closed** — code + tests in the repo at HEAD, with files anyone can read.
- **🟡 Deferred** — scope explicitly out for this milestone; recorded in `CHANGELOG.md`, the spec text, and this file. Closing them is a future PR, not a status change.
- **🔴 Open / actively wrong** — code that misrepresents itself. None right now (was 2 last pass; both retired this pass).
- **👤 Human-only** — actions no agent in this repo can take (outreach, record video). Not code-shaped, not blocking CI.

## Status of "close all gaps"

- **🟢 closed**: 19 of 23 — #1–4, #6, #8, #11–20, #22, #23.
- **🟡 deferred (named)**: 2 of 23 — #5 (PG schema parity catching up; engine works, 56/90 tables), #21 (PITR helpers landed, CLI wire deferred).
- **🔴 open**: 0.
- **👤 human-only**: 2 of 23 — #9 (outreach), #10 (screencast).

There are no silent stubs and no misleading rows left.

## Slim refactor (this pass)

- **53 → 36 crates (-32%)**: merged 4 reliability + 5 observability + 8 worker crates into 3 feature-flagged bundles; deleted 3 empty crates (`actant-napi`, `actant-wasm`, `actant-codegen-project`). No functionality lost — default features include every primitive; opt-out reduces dep tree without changing behavior.
- New **umbrella crates**: `crates/actantdb` (Rust, `cargo add actantdb`) and `packages/actantdb` (npm, `npm install actantdb`). Both are pure re-exports — one dep for all-in, feature-prune for storage-only / policy-only / etc.
- **target/** moved out of project tree to `~/.cache/cargo-actantdb` via `.cargo/config.toml` (gitignored). Debug builds were ballooning to 25–30 GB on this workspace; moved off-project + dropped `[profile.dev]` debuginfo to `line-tables-only` (-90% disk for debug builds).
