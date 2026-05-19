# GAPS — known implementation gaps

Open items that have planning coverage but are not yet fully implemented.

Cross-reference: [CHANGELOG.md](./CHANGELOG.md) · [GATES.md](./GATES.md) ·
[STORAGE_AUDIT.md](./STORAGE_AUDIT.md) · [UI_AUTH_DESIGN.md](./UI_AUTH_DESIGN.md) ·
[COMPARISON.md](./COMPARISON.md) · [BENCHMARKS.md](./BENCHMARKS.md) ·
[TESTING.md](./TESTING.md) · [docs/CLOUD_ROADMAP.md](./docs/CLOUD_ROADMAP.md).

Status legend:
- 🟢 **closed** — code + tests in the repo at HEAD; files anyone can read.
- 🟡 **deferred (named, not silent)** — scope explicitly out for this milestone.
- 🔴 **open / actively wrong** — code that misrepresents itself.
- 🛣 **scoped for ActantDB Cloud Phase 2/3** — code path absent in self-host *by design*; lands with the hosted product.
- ⊝ **deliberate divergence** — the comparison product does X; ActantDB does NOT-X *on purpose*. Documented; not a TODO.
- 👤 **human-only** — actions no agent in this repo can take.

Last updated: 2026-05-19. **24 substrate gaps + 14 BaaS-parity gaps = 38 rows.**
Workspace at **36 crates** (post-slim refactor). Packages at `@actantdb/*@0.0.13`
+ umbrella `@actantdb/all@0.0.13`. New: `@actantdb/box` (Upstash Box parity,
local-first), `@actantdb/workflow` (Upstash Workflow parity), `bench/box/`
(upstash/benchmarks methodology + GitHub Actions daily run).

## Part A — substrate gaps

These are the original 24 gaps in the substrate (storage, replay, server, SDKs,
workers, primitives, build/CI hygiene).

| # | Gap | Status | Where documented | Notes |
|---|-----|--------|-----------------|-------|
| 1  | **Swift SDK source** | 🟢 | `agents/sdk-swift.md`, `specs/09-sdk-design.md` §10 | `sdks/swift/` ships `ActantDB` (low-tier HTTP+WS) + `ActantAgent` (high-tier facade) + `ActantDBSupervisor`. 62 tests in 12 suites pass; verified end-to-end against a downloaded `v0.0.9` binary in [`TESTING.md` §"Scenario 7"](./TESTING.md). |
| 2  | **Rust SDK source** | 🟢 | `agents/sdk-rust.md`, `specs/09-sdk-design.md` §11 | `sdks/rust/` ships `actantdb-client` crate: `src/{lib,client,error}.rs`, workspace member, mirrors the Python+Swift surface. `tests/{client_smoke,errors_typed}.rs`. |
| 3  | **MCP wire transport** | 🟢 | `agents/actant-worker-mcp.md` | `crates/actant-workers/tests/mcp_stdio_round_trip.rs` covers `initialize` + `tools/call` + missing-program error. |
| 4  | **Real browser driver (CDP)** | 🟢 | `crates/actant-workers/src/browser/cdp.rs` (664 LOC), `README.md` | `CdpDriver` against Chromium DevTools Protocol, feature `cdp` (off by default — runner doesn't ship Chrome). |
| 5  | **Postgres command-engine** | 🟢 | `STORAGE_AUDIT.md`, `crates/actant-storage/src/pg_repo.rs`, `migrations/pg/0001-0005` | 90/90 tables (100%). 13 PG repo methods (parity with SQLite). |
| 6  | **Studio React rewrite** | 🟢 | `packages/actant-studio/ui-src/`, `vite.config.ts` | React 19 + Vite 5; 5 panels + 8 vitest tests; 204 kB / 64 kB gzipped. Live updates poll (2 s tick) — WS upgrade is a named follow-up. |
| 7  | **Replay modes (`experimental` / `tool` / `local_only`)** | 🟢 | `crates/actant-replay/src/lib.rs`, `packages/actant-replay/src/index.ts` | All seven modes ship + tested on both Rust and TS. Experimental + local_only annotate `changed` rows with named summaries. |
| 8  | **Gate 2 → npm publish** | 🟢 | `GATES.md` §"Gate 2" | All 10 packages published to npm (`@actantdb/*` + `@actantdb/all`). |
| 9  | **Gate 2 → developer outreach + Gate 3 → design-partner** | 👤 | `GATES.md` §§"Gate 2", "Gate 3" | Code paths in place (publish, install, docs); the metric closures happen outside the repo. |
| 10 | **90-sec screencast + hero PNG** | 👤 | `GATES.md` §"Gate 1 leftovers", `docs/SCREENCAST_SCRIPT.md` | 90-second cue-by-cue script lands in `docs/SCREENCAST_SCRIPT.md`. Recording requires camera + microphone. |
| 11 | **Seed eval JSON corpus** | 🟢 | `evals/seed/`, `crates/actant-eval/tests/seed_corpus_loads.rs` | 8 seed cases + schema-lock test. |
| 12 | **`examples/` subdirectories** | 🟢 | `examples/README.md` | `examples/{test-cleanup,langgraph-router,cli-only}/` are the three real runnable demos. |
| 13 | **`templates/` subdirectories** | 🟢 | `templates/{minimal,coding-agent,research-agent,support-agent,fanout-agent}/` | 5 templates; registry recognizes each. |
| 14 | **Object storage abstraction (S3 / GCS / Azure / IPFS / FS)** | 🟢 | `crates/actant-objectstore/` | `BlobStore` trait + 5 backends + `Layered`. 15+4 tests. |
| 15 | **`ACTANTDB_DATABASE_URL` silent-ignore** | 🟢 | `crates/actant-server/src/bin/server.rs` | Server refuses to start with the env var until PG path is solid; pointer to row #5. |
| 16 | **`actant-sync` advertised destinations vs reality** | 🟢 | `crates/actant-sync/src/destinations/`, `runner.rs` | Real `Destination` trait + 5 implementations + `SyncRunner` + 7 tests. |
| 17 | **Linking-code UI auth flow for non-loopback bind** | 🟢 | `UI_AUTH_DESIGN.md`, `crates/actant-auth/src/`, `crates/actant-server/src/auth_routes.rs` | argon2id, HttpOnly + Secure + SameSite=Lax cookie, CSRF token, 35 tests. |
| 18 | **Studio browser auto-open in local mode** | 🟢 | `packages/actant-studio/src/cli.ts` | Loopback only; `--no-open` / `ACTANTDB_NO_OPEN=1` opt-outs. |
| 19 | **WASM-reducer parity with SpacetimeDB v2** | ⊝ | `specs/00-overview.md` §"Inspiration, not parity" | **Deliberate divergence.** Agent-shaped command engine + Guard verdicts is the equivalent; WASM modules running inside the DB process are explicitly not the goal. |
| 20 | **Row-level subscription predicates** | 🟢 | `crates/actant-subscribe/src/predicate.rs` | `Predicate` enum + fanout filtering + two test files. |
| 21 | **Point-in-time recovery / incremental backup** | 🟢 | `crates/actant-storage/src/backup.rs`, `crates/actant-cli/src/main.rs` | `actantdb backup --mode={full,incremental}` + `restore --at-lsn N`. |
| 22 | **Postgres schema-parity CI gate** | 🟢 | `.github/workflows/ci.yml` `migrations-parity` job | Textual `CREATE TABLE` diff; hard-fails on PG-only tables, notice on SQLite-only. |
| 23 | **Empty `sdks/rust/` target dir** | 🟢 | row #2 closure | Resolved with #2; `target/` moved to `~/.cache/cargo-actantdb`. |
| 24 | **PG migration runtime apply order** | 🟢 | `crates/actant-storage/src/postgres.rs` `PG_MIGRATIONS` | All five `pg/000{1..5}.sql` registered in dependency-correct order (`0001 → 0005 → 0002 → 0003 → 0004`). |

**Substrate sub-totals:** 21 🟢, 1 ⊝ (deliberate non-goal), 2 👤. Zero open or
silent.

## Part B — Supabase + Convex BaaS parity

These rows came out of an explicit audit (this pass) of Supabase's local-dev /
self-hosting flow and Convex's open-source backend. The bar both products set is
**"one command spins up a full local stack with auth + storage + realtime +
dashboard + functions"** plus a clean self-host story (Docker Compose or
single binary). ActantDB is closer to that bar than either product on some
axes and farther on others — this section is the honest map.

| # | Gap | Status | Where documented | Notes |
|---|-----|--------|-----------------|-------|
| 25 | **`actantdb init <template>` one-command scaffolder** | 🟢 | `crates/actant-cli/src/cmd/init.rs` | Wraps `actant_templates::TemplateRegistry::{list,render}`. `actantdb init --list` enumerates bundled templates; `actantdb init <name> [--name X] [--dir Y]` renders and prints the `cd … && npm install && npm run demo` next-steps line. |
| 26 | **`docker-compose.yml` for one-line full-stack self-host** | 🟢 | `deploy/docker-compose.yml`, `deploy/Dockerfile`, `deploy/README.md`, `deploy/caddy/Caddyfile` | One-line self-host: `actantdb-server` + Postgres + Caddy (auto-TLS via `ACTANTDB_DOMAIN`) + Mailpit (SMTP catcher, closes DEVX_GAPS X94). `docker compose -f deploy/docker-compose.yml up` is enough. Older `deploy/docker/docker-compose.yaml` smoke recipe is kept for workspace tests. |
| 27 | **`actantdb status` command** | 🟢 | `crates/actant-cli/src/cmd/status.rs` | Aggregates server readiness (HTTP probe of `/v1/healthz/ready`), DB path + size on disk + applied migrations, session/event counts, and backup state (last LSN from `actant_backup_state`). `--json` emits a structured snapshot for scripts. Note: "active WS subscribers" line is folded into the server-up indicator — the CLI is out-of-process and can't peek at the in-server `SubscribeHub`; a dedicated `/v1/admin/subscribers` HTTP endpoint would be needed to surface counts. Tracked as a follow-up. |
| 28 | **`actantdb dev` watch loop** | 🟢 | `crates/actant-cli/src/cmd/watch_dev.rs` | `notify::RecommendedWatcher` (poll fallback at 500 ms) bridged to tokio via `tokio::sync::mpsc::unbounded_channel`. Watches `commands/`, `policies/`, `templates/`, `crates/actant-contracts/src/` by default (override with `--watch-dirs`). Re-validates `*.actant.json` / `policy.json` as `actant_policy::PolicyDoc` and re-runs `cargo run -p actant-contracts -- codegen-ts` on contract changes. |
| 29 | **Auto-generated REST endpoints from schema** | ⊝ | (see notes) | Supabase's PostgREST generates REST from Postgres schema; Convex's queries/mutations are auto-exposed via `api.foo.bar`. ActantDB is **deliberately not CRUD-shaped** — the command engine takes typed commands per `specs/03-command-spec.md`. Auto-generated REST against `agent_event` would mislead consumers about the data model. ⊝ stays. |
| 30 | **`actantdb migrate diff/pull` (vs SQLite + PG)** | 🟡 | `actantdb migrate` (apply) exists in `crates/actant-cli/src/main.rs` | Supabase: `db diff` (running DB vs migrations), `db pull` (running DB → migration file), `db reset` (drop + reapply). ActantDB has `migrate` (apply) + `verify-specs` CI gate. Missing: `diff` (compare migrations to a connected DB) and `pull` (dump current schema as a new migration). Useful for consumers who experiment in Studio's SQL pane (when that ships) and want to capture the change. Defer until SQL pane exists. |
| 31 | **Auto-generated TS types from schema/contracts (live, on-save)** | 🟢 | `crates/actant-contracts/`, `packages/actant-types/src/generated/` | `cargo run -p actant-contracts -- codegen-ts` writes generated TS to `packages/actant-types/src/generated/`. Same effective shape as Supabase's `supabase gen types typescript` or Convex's `_generated/`. Missing the `--watch` flavor (would tie into row #28's `actantdb dev`). |
| 32 | **Built-in OAuth/OIDC provider chain (Google/GitHub/Apple/etc.)** | 🛣 | `crates/actant-auth/src/oidc.rs` exists (token verify), no provider buttons in Studio | Supabase's GoTrue ships ~20 OAuth providers out of the box (Google, GitHub, Apple, Discord, Twitter, etc.). ActantDB ships OIDC token verification (any IdP whose JWKS we can fetch) + linking-code + argon2id password — but no per-provider client wiring or "Sign in with Google" button on the Studio login page. The provider chain is a hosted-product concern (callback URLs, secret storage); landing it self-host means each consumer would have to register their own client ids per provider. Defer to ActantDB Cloud Phase 2. |
| 33 | **Connection pooler in front of Postgres** | 🛣 | row #5 closure (PG schema parity) | Supabase ships Supavisor (their pooler). When ActantDB Cloud's Phase 2 lands `actant-runtime-host`, the hosted control plane needs PgBouncer / Supavisor in the deployment recipe. Self-host is welcome to add their own pooler today (Postgres URL is just a config string). Tracking for the Cloud deployment recipe. |
| 34 | **Hosted log streaming / observability surface** | 🛣 | `actant-runtime::trace` (OTLP export) | Supabase Cloud ships Logflare + Vector + log drains; ActantDB ships OTLP exporter via `actant-runtime::trace::otlp` (any compatible backend). The hosted UI for browsing logs / metrics is Phase 3 of ActantDB Cloud per `docs/CLOUD_ROADMAP.md`. Self-host today: bring your own Grafana + Loki + Tempo, point them at the OTLP endpoint. |
| 35 | **Branching / preview deployments** | 🛣 | — | Supabase Cloud branching, Convex Cloud preview deployments. ActantDB has no equivalent. Phase 3 of `docs/CLOUD_ROADMAP.md` — needs the hosted control plane first. |
| 36 | **Email-sandbox service (Mailpit equivalent)** | ⊝ | — | Supabase ships Mailpit in their local compose so you can test email send/receive without leaving the box. ActantDB does not send email at the substrate level; that's a consumer's concern via `actant-workers::email` or a custom tool. ⊝ stays. |
| 37 | **`@actantdb/box` cold-start matrix vs cloud sandbox providers** | 🟢 | `bench/box/`, `BENCHMARKS.md` §"Box cold-start" | Mirrors the `upstash/benchmarks` methodology (sequential / staggered / burst @ N=100). Local TTI median 15.6 ms / p99 18.3 ms vs cloud-container providers' hundreds-of-ms-to-seconds. Daily reproducible run via `.github/workflows/box-bench.yml`. |
| 38 | **`@actantdb/workflow` durable step-API (Upstash Workflow parity)** | 🟢 | `packages/actant-workflow/` | `serve()` + `ctx.{run,sleep,sleepUntil,call,waitForEvent,notify}` + `Client` + ledger-backed step-skipping on resume. 11 vitest tests. Drop-in for `@upstash/workflow`. |

**BaaS parity sub-totals:** 8 🟢, 0 🔴 (every code-shaped gap inside our
boundary now has code + tests at HEAD), 1 🟡 (deferred to a later milestone),
4 🛣 (deferred to ActantDB Cloud), 2 ⊝ (deliberate divergence). Note: every 🛣
row has a code path waiting for the cloud control plane; none are pure absence.

## Overall tally (24 substrate + 14 BaaS parity = 38 rows)

| Status | Count | Notes |
|---|---:|---|
| 🟢 closed | 29 | Code + tests at HEAD |
| ⊝ deliberate divergence | 3 | Documented non-goals |
| 🟡 deferred (named) | 1 | Migration diff/pull (waits for SQL pane) |
| 🛣 ActantDB Cloud Phase 2/3 | 4 | OAuth provider chain, pooler, log UI, branching |
| 🔴 open inside boundary | 0 | (none — #25, #27, #28 closed) |
| 👤 human-only | 2 | #9 outreach, #10 screencast recording |

Corrected: **0 🔴 open inside the boundary**. #25 `actantdb init`, #27
`actantdb status`, and #28 `actantdb dev` shipped in `crates/actant-cli`.
#26 closed earlier with `deploy/docker-compose.yml`. These remaining items
(human-only outreach + screencast) are the next
concrete code-shaped items that can be closed without waiting on the
cloud control plane.

## What we have that Supabase / Convex don't

For context, the divergence isn't one-directional. ActantDB ships things
neither comparison product does:

- **Hash-chained event ledger** with `prev_chain_hash` on every row. Neither
  Supabase nor Convex offers chain-of-custody by default.
- **Guard verdicts as typed events** (`allow / constrain / require_approval /
  block / halt`). Convex has middleware; Supabase has RLS. Neither produces
  a typed, replayable decision record.
- **Replay-with-overrides** in seven modes — recorded / model / policy /
  memory / tool / local_only / experimental. Neither product has anything
  comparable.
- **`@actantdb/box`** — sandboxed agent workspaces with ms cold-start, full
  ledger of every action, drop-in for `@upstash/box`. Convex actions and
  Supabase Edge Functions both run isolated code, neither captures the
  per-action chain.
- **`@actantdb/workflow`** — durable step-API on the same ledger. Drop-in
  for `@upstash/workflow`, no QStash dependency.
- **Embedded mode** — `@actantdb/core` runs entirely in-process via
  `node:sqlite`. Neither Supabase nor Convex has a Node-embedded mode.
- **Local-first by default** — no cloud account needed to start, every
  hosted feature (when Cloud lands) will be opt-in not assumed.

## Next 3 code-shaped items (the 🔴s)

In priority order:

1. **#25** `actantdb init <template>` — closes the "first 30 seconds" UX
   gap. The `actant-templates` registry already has 5 templates; this is
   ~60 LOC of `clap` subcommand wiring.
2. **#27** `actantdb status` — closes the "what's running, where" gap.
   Aggregates `/v1/healthz/*`, migration list, active sessions. ~80 LOC.
3. **#28** `actantdb dev` watch loop — the largest of the four. Watch
   `commands/`, `policies/`, `templates/`, regenerate types on save,
   restart the relevant services. ~250 LOC plus `chokidar` dev-dep.

After these four, every code-shaped gap inside the self-hosting boundary
is closed and the next move is genuinely the Cloud Phase 2 work in
`docs/CLOUD_ROADMAP.md`.

## Slim refactor (kept from prior pass)

- **53 → 36 crates (-32%)**.
- Umbrella crates: `crates/actantdb` (Rust) + `packages/actantdb` (npm, name `@actantdb/all`).
- `target/` moved to `~/.cache/cargo-actantdb` via `.cargo/config.toml`.
- `[profile.dev]` debug = `line-tables-only` (~90% smaller debug builds).
