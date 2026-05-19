# GAPS — known implementation gaps

Open items that have planning coverage but are not yet fully implemented.
Cross-reference: [CHANGELOG.md](./CHANGELOG.md) · [GATES.md](./GATES.md) ·
[STORAGE_AUDIT.md](./STORAGE_AUDIT.md) · [UI_AUTH_DESIGN.md](./UI_AUTH_DESIGN.md) ·
[COMPARISON.md](./COMPARISON.md) · [BENCHMARKS.md](./BENCHMARKS.md) · [TESTING.md](./TESTING.md).

Status legend: 🟢 closed · 🟡 deferred (named, not silent) · 🔴 open / actively wrong · 👤 human-only

Last updated: 2026-05-19, packages at `@actantdb/*@0.0.9`, release tag `v0.0.9`.

## Status table

| # | Gap | Status | Where documented | Notes |
|---|-----|--------|-----------------|-------|
| 1  | **Swift SDK source** | 🟢 | `agents/sdk-swift.md`, `specs/09-sdk-design.md` §10 | `sdks/swift/` ships `ActantDB` (low-tier HTTP+WS) + `ActantAgent` (high-tier facade) + `ActantDBSupervisor`. 62 tests in 12 suites pass; verified end-to-end against a downloaded `v0.0.9` binary in [`TESTING.md` §"Scenario 7"](./TESTING.md). |
| 2  | **Rust SDK source** | 🟡 | `agents/sdk-rust.md`, `specs/09-sdk-design.md` §11 | `sdks/rust/` directory exists but contains only `target/` — no real client crate. Consumers use the HTTP API directly or the Rust workspace crates by path. Re-scaffold if a Rust consumer asks. |
| 3  | **MCP wire transport** | 🟢 | `agents/actant-worker-mcp.md` | `crates/actant-worker-mcp/tests/stdio_round_trip.rs` covers `initialize` + `tools/call` round-trip + missing-program error. |
| 4  | **Real browser driver** | 🟡 | CHANGELOG §Deferred | `EmulatorDriver` ships; CDP driver gated behind `--features cdp` (off by default — see `crates/actant-worker-browser/Cargo.toml`). Phase-2 deferral; the gating is honest now. |
| 5  | **Postgres command-engine** | 🔴 | `STORAGE_AUDIT.md` §"Backend completeness" + this row supersedes the old 🟡 | `PgStorage` is a connection wrapper. Migration `migrations/pg/0001_initial.sql` ships **7 of 87 tables** (workspace / actor / session / message / agent_event / command_record / idempotency_record). 8 downstream crates (`actant-memory`, `actant-lock`, `actant-replay`, `actant-flow`, `actant-server`, `actant-ingress`, `actant-audit-export`, `actant-tenant`) hardcode `&Storage` (the SQLite type) — Postgres can't reach them even in principle. Server now **fails loud** on `ACTANTDB_DATABASE_URL` instead of silently downgrading to SQLite (see #13). Genuine multi-writer support is the largest remaining substrate gap. |
| 6  | **Studio React rewrite** | 🟡 | CHANGELOG §Deferred | Vanilla JS wedge ships and works. React rewrite still post-design-partner. |
| 7  | **`experimental` / `tool` / `local_only` replay modes** | 🟡 | `specs/07-workflows-and-replay.md`, CHANGELOG §Deferred | Named-error stubs. Require replay-scoped worker re-invocation. Phase-5 follow-up. |
| 8  | **Gate 2 → npm publish** | 🟢 | `GATES.md` §"Gate 2" §"What humans must do" | All 8 `@actantdb/*` packages published to npm under `latest` + `shadow` tags as of `0.0.2`, currently at `0.0.9`. CI workflow at [`.github/workflows/publish-npm.yml`](./.github/workflows/publish-npm.yml). |
| 9  | **Gate 2 → developer-outreach + Gate 3 → design-partner** | 👤 | `GATES.md` §§"Gate 2", "Gate 3" | Blocked on human outreach. No agent action closes these. |
| 10 | **90-sec screencast + hero PNG** | 👤 | `GATES.md` §"Gate 1 leftovers" | Human-produced artifacts. |
| 11 | **Seed eval JSON corpus** | 🟡 | `agents/actant-eval.md` | DSL + `Criterion` + `SuccessCriteria` ship in `actant-eval`; the populated `evals/seed/` corpus does not. Phase 4 deferral, scoped by `agents/actant-eval.md`. |
| 12 | **`examples/` subdirectories** | 🟡 | originally `examples/README.md` | Replaced by `wedge/demo`, `wedge/demo-langgraph`, `wedge/demo-cli`. Re-create `examples/` when there's a second-framework adapter to demo. |
| 13 | **`templates/` subdirectories** | 🟡 | `templates/README.md` | `actant-templates` ships `minimal` + `coding-agent` (verified via `cargo test -p actant-templates` — 19 passing). The originally-listed 9 named templates remain a packaging decision deferred until there's a consumer requesting one. |
| 14 | **Object storage abstraction (S3 / GCS / Azure / IPFS / FS)** | 🟢 | this entry replaces "no object storage anywhere" from STORAGE_AUDIT.md gap #4 | `crates/actant-objectstore/` ships the `BlobStore` trait + `FilesystemStore` (default, 2-char-prefix sharding) + `MemoryStore` (tests) + `S3Store` (feature `s3` via `object_store::aws::AmazonS3`, includes presign) + `IpfsStore` (feature `ipfs` against Kubo `/api/v0/*`) + `Layered` (URI-scheme routing). `Storage::put_artifact(...)` writes via the injected store and inserts the artifact row. 15 new objectstore tests + 4 new storage tests. S3 / IPFS are opt-in via feature flag — default build pulls neither dep. |
| 15 | **`ACTANTDB_DATABASE_URL` silent-ignore** | 🟢 | this entry was STORAGE_AUDIT.md gap #1 | `crates/actant-server/src/bin/server.rs` now refuses to start if `ACTANTDB_DATABASE_URL` is set (with password redacted in the error). Pointer to gap #5 above so the user knows why. Helm `storage.backend=postgres` will now surface the gap immediately instead of looking healthy on SQLite. |
| 16 | **`actant-sync` advertised destinations vs reality** | 🟡 | `crates/actant-sync/README.md`, `agents/actant-sync.md` | The crate's docs advertise S3 / GCS / Azure destination implementations; the source is a 58-line file with one function. The infrastructure to ship these destinations now exists (gap #14), but `actant-sync` itself still needs to grow real `Destination` impls + retry / backpressure. Phase-6-or-later. |
| 17 | **Linking-code UI auth flow for non-loopback bind** | 🟢 | `UI_AUTH_DESIGN.md`, `crates/actant-auth/src/{link,password,session}.rs`, `crates/actant-server/src/auth_routes.rs` | Loopback bind = trust the OS user, no password. Non-loopback bind = refuse to start without TLS *or* `--insecure-public`; print a one-time `xxxx-xxxx-xxxx` linking code (60-bit entropy, 15-min TTL, sha256-stored). `/v1/auth/{link,password,login,logout,me}` endpoints; argon2id password; `Set-Cookie: actantdb_session=... HttpOnly; Secure; SameSite=Lax` + `X-CSRF-Token` required on mutating routes. Migration `0004_auth.sql` adds `workspace_owner`, `link_code`, `session_token`. Tests in `crates/actant-server/tests/{auth,link_code_flow,password_set_and_login,csrf_required_on_mutate}.rs`. |
| 18 | **Studio browser auto-open in local mode** | 🟢 | `packages/actant-studio/src/cli.ts` `openBrowser` | `actantdb studio` auto-opens the default browser on loopback URLs (macOS `open`, Linux `xdg-open`, Windows `cmd /c start`). Opt out via `--no-open` or `ACTANTDB_NO_OPEN=1`. Spawn errors are swallowed (slim containers won't crash). |
| 19 | **WASM-reducer parity with SpacetimeDB v2** | 🟡 | `specs/00-overview.md` §"Inspiration, not parity" | Explicit non-goal. ActantDB's typed Rust command engine plus Guard verdicts is the agent-shaped equivalent. Re-evaluate if a consumer asks for user-supplied WASM modules running inside the DB process. |
| 20 | **Row-level subscription predicates** | 🟡 | `crates/actant-subscribe/src/lib.rs` | Current subscribe is topic-keyed (`Topic { workspace_id, session_id, kind }`); SpacetimeDB-style `SELECT … WHERE …` row-level filters do not exist. The kind-keyed broadcast is sufficient for agent timelines; add real predicates once a consumer hits the limit. |
| 21 | **Point-in-time recovery / replication** | 🟡 | `STORAGE_AUDIT.md` §"Backup / restore" | `actantdb backup` does `wal_checkpoint(TRUNCATE)` + file copy (consistent snapshot). No incremental WAL shipping, no streaming replica, no read-replica path. Deferred until a production deployment needs RPO < daily-snapshot. |
| 22 | **Postgres schema parity check in CI** | 🔴 | `STORAGE_AUDIT.md` §"Backend completeness" | No CI gate catches that `migrations/pg/*.sql` diverges from `migrations/*.sql`. Currently 7 vs 87 tables. Once Postgres is real (gap #5) the migration-parity gate becomes the equivalent of the `verify-specs` gate we ship today. |
| 23 | **`sdks/rust/` empty target dir** | 🟡 | row #2 above | The directory is just a leftover build artifact — no Cargo.toml, no Rust SDK code. Either delete it or scaffold a real client crate when a consumer asks. |

## What "100% complete" means

- **🟢 Closed gaps** — code + tests in the repo at HEAD, with named files/tests anyone can read.
- **🟡 Named deferrals** — scope explicitly out for this milestone; recorded in `CHANGELOG.md`, the spec text, and this file. Closing them requires a future PR, not a status change.
- **🔴 Open / actively wrong** — code in the tree that *misrepresents* itself (e.g., PgStorage looks usable but isn't). These are the highest-priority items even though they're not "missing"; they're worse than missing because they mislead.
- **👤 Human-only** — actions that no agent in this repo can take (outreach, record video, sign a design-partner contract).

A "100% green" snapshot is artifact-shaped: every 🟢/🟡/🔴/👤 item is *known* and *documented*; no silent stubs remain. The 🟡 / 👤 rows do not block validation gates — they block product evolution past the wedge. The 🔴 rows block claims the README is currently making.

## What changed in this revision vs the prior GAPS.md

- **Closed** (🟢): #1 (Swift SDK + supervisor), #8 (npm publish), #14 (object storage), #15 (`ACTANTDB_DATABASE_URL` silent-ignore), #17 (UI auth linking flow), #18 (browser auto-open).
- **Reclassified to 🔴** (worse than I admitted before): #5 (Postgres command-engine) — was "deferred", is actually "deceptive": README claims a Postgres backend that exists in name only. #22 (Postgres schema parity in CI) — newly identified.
- **Added**: #14–23 (the storage audit + UI auth + SpacetimeDB framing + Rust SDK directory all surfaced gaps the prior table didn't name).
- **No longer cross-referenced**: `planning/eval-catalog.md` (removed earlier); `dist-publish/*.tgz` (removed; superseded by the `publish-npm.yml` workflow).
