# DEVX_GAPS — local-deployment developer experience

The substrate is solid (see [GAPS.md](./GAPS.md)) and the cloud roadmap is
mapped ([CLOUD_GAPS.md](./CLOUD_GAPS.md)). What's *not* yet
explicitly tracked: the **developer experience** of using ActantDB locally,
specifically for **AI-agent devs who already have a stack** and need to drop
us in without rewriting their app.

This file is the audit. Same status taxonomy as `CLOUD_GAPS.md`:
🟢 ships / 🚧 partial / 🔴 missing / ⊝ deliberate non-goal.

Cross-reference: [GAPS.md](./GAPS.md), [CLOUD_GAPS.md](./CLOUD_GAPS.md),
[COMPARISON.md](./COMPARISON.md), [TESTING.md](./TESTING.md).

Last updated: 2026-05-20.

---

## Part A — First-touch onboarding (0 → first-event)

The "I just heard about this; can I get something running in 5 minutes" path.
Everything here is table-stakes for 2026; the bar is set by Next.js, Vite,
Astro, Convex, and Supabase.

| # | Gap | Status | Notes |
|---|-----|--------|-------|
| X1 | **`npm create actantdb` / `npx @actantdb/create-app`** | 🟢 | `packages/create-actantdb/`. Interactive (`prompts`+`kleur`) and headless (`--template <name> --framework <name> --no-interactive`) modes. 9 vitest tests. Verified end-to-end: `node packages/create-actantdb/dist/index.js test-scaffold --template minimal --framework mastra --no-interactive` produces a valid scaffold. |
| X2 | **First-launch Studio "welcome" screen** | 🟢 | `packages/actant-studio/ui-src/panels/RunsPanel.tsx` now renders a first-run empty state with a copy-pasteable `@actantdb/mastra` snippet that captures one tool call. Covered by Studio UI tests. |
| X3 | **`actantdb doctor`** | 🟢 | Shipped in `crates/actant-cli/src/cmd/doctor.rs`. Checks rustc ≥ 1.88, node ≥ 22.5, disk space (5 GB threshold) on the db dir's filesystem, ports 4555 + 54323 (prints PID via `lsof`), optional `claude`/`codex`/`opencode` on PATH, `ACTANTDB_DATABASE_URL` shape, and Studio `dist/ui/` presence. Each check prints `[ok]/[warn]/[fail]` + a one-line fix where applicable. |
| X4 | **Pretty errors with one-line fixes** | 🚧 | We have `BoxError` with typed codes (see `@actantdb/box`). Many SDK / CLI errors are still raw `Error.message`. Need an `ActantError` base with `code`, `hint`, `fix_command?` shape applied to every public throw. |
| X5 | **Interactive 5-minute tutorial** | 🔴 | Convex's quickstart is a clickable 5-step tutorial. Ours is a static markdown demo. Could be a Stackblitz / Replit / CodeSandbox link from the README that opens with `@actantdb/all` preinstalled. |
| X6 | **CLI shell completion** | 🟢 | Hidden `actantdb completions <shell>` subcommand wired through `clap_complete::generate` in `crates/actant-cli/src/main.rs`. Supports bash/zsh/fish/elvish/powershell. |
| X7 | **First-run telemetry opt-in (truthful)** | 🔴 | One prompt on first `actantdb` invocation: "share anonymous usage so we can fix what breaks?" with a clear opt-out path. Convex + Vercel do this. Don't be sneaky; the prompt itself is the trust-builder. |

**Part A: 4 🟢 / 1 🚧 / 2 🔴.** X1, X2, X3, and X6 now cover the first-create,
first-open, doctor, and shell-completion path. X5 and X7 remain DX polish.

---

## Part B — Framework adapters (plug into what they're already using)

The user said: *"easily compatible with what users may already be using"*.
Here's the map.

| # | Framework | Status | Notes |
|---|---|--------|-------|
| X8  | **Mastra** | 🟢 | `@actantdb/mastra` ships `withActant()`. The canonical adapter. |
| X9  | **LangGraph** | 🚧 | Demo at `examples/langgraph-router/` works, but there's no `@actantdb/langgraph` dedicated package. Most LangGraph users will look for one by name. Need: `packages/actant-langgraph/` re-exporting `withActant` with LangGraph-idiomatic naming + a `LangGraphNode` wrapper. |
| X10 | **AI SDK by Vercel** | 🟢 | `packages/actant-ai-sdk/` ships as `@actantdb/ai-sdk` with workspace tests. |
| X11 | **OpenAI Agents SDK** (`@openai/agents`) | 🟢 | `packages/actant-openai-agents/` ships as `@actantdb/openai-agents` with workspace tests. |
| X12 | **Anthropic SDK direct** (`@anthropic-ai/sdk`) | 🟢 | `packages/actant-anthropic/` ships as `@actantdb/anthropic` with workspace tests. |
| X13 | **OpenAI SDK direct** | 🟢 | `packages/actant-openai/` ships as `@actantdb/openai` with workspace tests. |
| X14 | **CrewAI** (Python) | 🔴 | Python — our SDK exists. Need a `crewai-actantdb` package on PyPI with a `with_actant_logging(crew)` decorator. |
| X15 | **AutoGen** (Microsoft, Python) | 🔴 | Same shape; Python. |
| X16 | **LangChain JS** | 🟢 | `packages/actant-langchain/` ships as `@actantdb/langchain` with workspace tests. |
| X17 | **LangChain Python** | 🔴 | Same; Python pip package. |
| X18 | **Inngest** | 🔴 | Inngest is the canonical durable-workflow alt to QStash. `@actantdb/inngest` middleware that logs every step as a ledger event. |
| X19 | **Trigger.dev** | 🔴 | Same shape as Inngest. |
| X20 | **Vercel AI Gateway** | 🔴 | If a user proxies through Vercel AI Gateway, we should record `model_call` events with gateway routing metadata intact. |
| X21 | **Ollama / local models** | 🚧 | `actant-runtime::models` registry knows about Ollama. The `withActant` wrapper sees the model name. Need explicit guidance in docs + a `examples/ollama-only/` demo. |
| X22 | **Convex** | 🟢 | `@actantdb/convex` exists and its package tests passed in the current workspace run. A public smoke demo is still useful, but the package is no longer untested. |
| X23 | **Supabase** | 🔴 | The opposite direction — adapter for *running ActantDB inside a Supabase Edge Function* so a Supabase consumer can add ActantDB without standing up a separate server. Worth shipping once GAPS row #26 (docker-compose) ships. |

**Part B: 7 🟢, 2 🚧, 7 🔴, 0 ⊝.** The highest-volume TypeScript adapters now
exist. Remaining red rows are mostly Python framework adapters, durable
workflow middleware, gateway-specific metadata capture, and a Supabase edge
adapter.

---

## Part C — Runtimes / environments

Where can you actually run ActantDB *embedded* today vs where you'd need
the server?

| # | Runtime | Status | Notes |
|---|---|--------|-------|
| X24 | **Node ≥ 22.5** (`node:sqlite`) | 🟢 | First-class runtime. `@actantdb/core` runs embedded. |
| X25 | **Bun** | 🚧 | Probably works (Bun is Node-compatible) but never tested. Need: smoke test in CI matrix. |
| X26 | **Deno** | 🔴 | No `node:sqlite`. Would need a Deno-native SQLite (`@db/sqlite`) variant of `@actantdb/core` or HTTP-only mode (`@actantdb/sdk`). Most Deno-targeted agent devs would accept HTTP-only. |
| X27 | **Cloudflare Workers** | 🔴 | No filesystem, no `node:sqlite`. Server mode via fetch only. Future-proof path: a `@actantdb/cloudflare` adapter that wraps a Durable Object + R2 for the ledger. Big lift. Defer until requested. |
| X28 | **Vercel Edge / Next.js Route Handlers (edge)** | 🔴 | Same constraints as Workers. Server-mode via `fetch` works today; embedded does not. |
| X29 | **Browser** | ⊝ | We don't run in browser (no `node:sqlite`). Users connect via `@actantdb/sdk` over HTTP+WS. Documented non-goal. |
| X30 | **Native Mac / iOS via Swift SDK** | 🟢 | `sdks/swift/` ships `ActantDB` (HTTP+WS) + `ActantAgent` (high-tier) + `ActantDBSupervisor` (spawn the actantdb-server subprocess). Closed via GAPS row #1. |
| X31 | **Native Android via Kotlin** | 🔴 | Java/Kotlin SDK missing entirely. Android agent dev is small today but growing. |

**Part C: 2 🟢, 1 🚧, 4 🔴, 1 ⊝.** The embedded-runtime list is what's
fundamentally limiting "drop into my existing project". Bun (X25) and Deno
(X26) are easy wins; Edge runtimes (X27, X28) are big.

---

## Part D — Developer tooling beyond Studio

Things the developer reaches for that aren't the browser dashboard.

| # | Tool | Status | Notes |
|---|---|--------|-------|
| X32 | **`actantdb tail`** | 🟢 | `crates/actant-cli/src/cmd/tail.rs` — DB-polling (500 ms) `tail -f` with `--session/--kind/--actor` filters and `-f` follow mode. Inline pretty-prints `tool_call_*` (tool + status) and `model_call*` (model + tokens). |
| X33 | **`actantdb watch <predicate>`** | 🟢 | `crates/actant-cli/src/cmd/watch.rs` + `crates/actant-cli/src/predicate_parse.rs` — hand-rolled recursive-descent parser producing `actant_subscribe::Predicate`, evaluated against each new event row. Polls the DB (CLI is out-of-process, can't share the in-server `SubscribeHub`). |
| X34 | **`actantdb shell` REPL** | 🟢 | `crates/actant-cli/src/cmd/shell.rs` — rustyline-backed read-only REPL with `events`, `sessions`, `get <id>`, `help`, `exit` commands. Renders tables via `comfy-table`. |
| X35 | **`actantdb explain <event_id>`** | 🟢 | `crates/actant-cli/src/cmd/explain.rs` — walks `parent_event_id` + `causal_parent_ids` (the JSON array added in migration 0002) backwards and the `parent_event_id` index forwards, plus surfaces tool/model call ids and `status`/`took_ms` from the inline payload. |
| X36 | **`actantdb sql`** | 🟢 | `crates/actant-cli/src/cmd/sql.rs` — opens via `SqliteConnectOptions::read_only(true)` AND refuses any first token other than `SELECT`/`WITH` AND refuses semicolons outside string literals. Pretty-prints via `comfy-table`. 4 unit tests. |
| X37 | **`actantdb export`** | 🟢 | `crates/actant-cli/src/cmd/export_import.rs::run_export` — JSON / NDJSON / CSV. Parquet deferred (would require pulling in `arrow` + `parquet`; noted as follow-up). Sensitivity ceiling: rows with `sensitivity == "secret"` have `payload_inline` replaced with `"<redacted: secret>"`. |
| X38 | **`actantdb import`** | 🟢 | `crates/actant-cli/src/cmd/export_import.rs::run_import` — reads NDJSON (the canonical export format) and inserts via `INSERT OR IGNORE`. Idempotency: refuses if any imported `session_id`/`workflow_run_id` already has events in the target DB. |
| X39 | **VSCode extension** | 🔴 | Inline event count next to function declarations. Click a function → see the events it produced. Hover a `withActant`-wrapped call → see the recent verdicts. Big leverage for usage. |
| X40 | **Cursor / Windsurf / Copilot rules** | 🟢 | `.cursorrules`, `.windsurfrules`, and `.github/copilot-instructions.md` ship the same workspace shape + binding-rules brief so each AI coding assistant has the right priors. |
| X41 | **Browser DevTools extension** | 🔴 | For inspecting `@actantdb/sdk` WebSocket subscriptions in dev. Network panel already shows them; this would parse + decode. Niche. |

**Part D: 7 🟢 / 3 🔴.** CLI subcommands X32–X38 shipped (see
`crates/actant-cli/src/cmd/`). VSCode extension (X39) + browser DevTools
panel (X41) + per-IDE rules pass-throughs remain.

---

## Part E — Language SDKs

Coverage of the SDK matrix. We ship TS, Python, Swift, Rust. Comparison
products typically ship 6–10 languages.

| # | Language | Status | Notes |
|---|---|--------|-------|
| X42 | **TypeScript** (`@actantdb/sdk`) | 🟢 | Core surface, well-typed, generated from `actant-contracts`. |
| X43 | **Python** (`sdks/python/actantdb`) | 🚧 | Ships; **no `.pyi` type stubs**, no async client (just blocking), no `asyncio` flavor. Modern Python agent devs expect `httpx.AsyncClient`-style. |
| X44 | **Swift** (`sdks/swift`) | 🟢 | Full `ActantDB` + `ActantAgent` + supervisor. Closed via GAPS #1. |
| X45 | **Rust** (`sdks/rust`, `actantdb-client`) | 🟢 | Workspace member; covers HTTP + WS subscribe; mirrors Python+Swift surface. Closed via GAPS #2. |
| X46 | **Go** | 🔴 | Missing. Go is the second-largest substrate language for new infra; many ops teams default to it. |
| X47 | **Java / Kotlin** | 🔴 | Missing. Enterprise JVM shops + Android. |
| X48 | **.NET / C#** | 🔴 | Missing. Microsoft AI customers. |
| X49 | **Ruby** | 🔴 | Missing. Rails community is loud and present. |
| X50 | **PHP** | 🔴 | Missing. Laravel ecosystem has growing AI usage. |
| X51 | **Elixir** | 🔴 | Missing. Small but enthusiastic; supabase-realtime is Elixir, so there's overlap. |

**Part E: 3 🟢, 1 🚧, 6 🔴.** Priority order by ROI: Go (X46) > async
Python (X43) > Kotlin (X47) > .NET (X48). PHP / Ruby / Elixir can wait
for explicit demand.

---

## Part F — Observability & data flow integrations

Pre-built bridges to popular observability + data tools so an ops team
can wire ActantDB into what they already operate.

| # | Integration | Status | Notes |
|---|---|--------|-------|
| X52 | **OpenTelemetry exporter** | 🟢 | `actant-runtime::trace::otlp` ships. Works with any OTLP-compatible backend (Jaeger, Tempo, Honeycomb, etc.). |
| X53 | **`/metrics` Prometheus endpoint** | 🟢 | `crates/actant-server/src/prom.rs` exposes the in-process registry at `/metrics` alongside the older snapshot view at `/v1/metrics`. Ships with `actant_commands_dispatched_total`, `actant_http_request_duration_seconds`, `actant_ledger_bytes`. Remaining metrics (`actant_events_appended_total`, `actant_active_sessions`, `actant_subscribe_active`) need wiring inside `actant-storage` / `actant-subscribe` and are tracked as a follow-up. |
| X54 | **Sentry integration** | 🔴 | Auto-emit Sentry events for `tool_call_completed { status: "error" }`. |
| X55 | **PostHog product analytics** | 🔴 | For consumer apps using ActantDB to track agent usage. |
| X56 | **Datadog APM** | 🔴 | OTLP already gets us most of the way; this is "log a partner" certification. |
| X57 | **Honeycomb integration** | 🔴 | OTLP works; "Honeycomb-certified" badge if their program admits us. |
| X58 | **Langfuse / LangSmith / Helicone (trace UIs)** | 🔴 | These products are the "look at agent traces" alternative. We compete; we *also* support exporting to them via `actant-sync` for shops that already standardized on one. |
| X59 | **dbt models from the ledger** | 🔴 | Ship a `dbt-actantdb` package with starter models (`events_by_actor`, `runs_per_workspace_per_day`). The data team's analyst can join with their warehouse data immediately. |
| X60 | **Apache Superset / Metabase** | 🔴 | Pre-built dashboards (JSON exports) for someone to load into their existing BI. |
| X61 | **Snowflake / BigQuery / Databricks export** | 🚧 | `actant-sync` does S3/GCS/Azure/IPFS already. Pure SQL warehouse loaders need 5 more lines each (run a `COPY INTO` periodically). |
| X62 | **MLflow / W&B** | 🔴 | For ML-eval shops. The replay / eval surface is comparable to MLflow's experiment-tracking. Adapter ships eval events as MLflow runs. |

**Part F: 1 🟢, 2 🚧, 9 🔴.** OpenTelemetry (X52) covers most ops teams.
The 🔴s here are mostly "the customer asked → ship the adapter" work.

---

## Part G — MCP integration (let agents query the ledger)

ActantDB currently *calls* MCP servers (`actant-workers::mcp`). The
inverse — exposing the ActantDB ledger *as* an MCP server so an agent
(Cursor, Claude Desktop, etc.) can ask "what did I do yesterday?" — is
absent.

| # | Item | Status | Notes |
|---|---|--------|-------|
| X63 | **`@actantdb/mcp-server`** | 🟢 | `packages/actant-mcp-server/` ships stdio + HTTP transports. 8 tools: `list_runs`, `get_event`, `list_events`, `query_predicate`, `replay`, `list_pending_approvals`, `decide_approval`, `get_workspace_summary`. 11 vitest tests. Verified: stdio `initialize` round-trips. |
| X64 | **MCP resource discovery** | 🟢 | `packages/actant-mcp-server/src/resources.ts` exposes `actant://workspace/{ws}/session/{sid}` + `actant://workspace/{ws}/runs` URI templates via the MCP resources protocol. |
| X65 | **One-click "Add to Claude Desktop"** | 🟢 | Root `README.md` "Integrations" section ships the `claude_desktop_config.json` snippet. Future enhancement: hosted button on the website (Phase 2 cloud). |

**Part G: 3 🟢 / 0 🔴.** X63 is the breakthrough item. Cursor + Claude
Desktop usage of ActantDB depends on this existing.

---

## Part H — Documentation, recipes, content

Where someone goes to learn a pattern.

| # | Item | Status | Notes |
|---|---|--------|-------|
| X66 | **API reference (typedoc / rustdoc)** | 🟢 | `rustdoc` autogens on the Rust side. `typedoc.json` at the repo root + `pnpm typedoc` script render every `packages/actant-*/src/index.ts` into `docs/api-typescript/`. |
| X67 | **`docs/recipes/`** | 🟢 | `docs/recipes/` ships an index README + 10 recipes: 01 approval, 02 replay-failed-run, 03 Next.js wiring, 04 Ollama-only, 05 snapshot testing, 06 BigQuery export, 07 share-a-replay-session, 08 audit-export-to-S3, 09 add-to-existing-mastra-app, 10 first-MCP-tool-on-top-of-ActantDB. |
| X69 | **"Awesome ActantDB"** list | 🔴 | Curated list of community examples once we have any. Empty for now; can seed with our 3 demos + 5 templates. |
| X70 | **Migration guides FROM other tools** | 🔴 | "Migrating from Langfuse to ActantDB" / "Adding ActantDB on top of your Inngest workflows" / "Replacing in-house logging with the ActantDB ledger". Inbound-marketing gold. |
| X71 | **Interactive playground** (Stackblitz / CodeSandbox) | 🔴 | Embed in the README. Visitor can click "Run" without installing anything. |
| X72 | **Architecture diagrams** | 🚧 | `specs/` has ASCII diagrams. None of them render well on GitHub or docs sites. Need: SVG diagrams committed alongside, or Mermaid blocks. |

**Part H: 2 🟢, 1 🚧, 3 🔴.**

---

## Part I — Testing tools for consumers

Things our users need to test *their* agent code that integrates with us.

| # | Item | Status | Notes |
|---|---|--------|-------|
| X73 | **`@actantdb/testing`** | 🟢 | `packages/actant-testing/` exports `createTestLedger`, `expectEventEmitted`, `expectEventNotEmitted`, `expectGuardVerdict`, `expectToolCompleted`, `expectChainIntact`, `findEvents`, `snapshotEvents`, `AssertionError`. In-memory ledger; no `~/.actantdb` touch. 10 vitest tests. |
| X74 | **Snapshot testing** | 🟢 | `snapshotEvents(ledger)` in `@actantdb/testing` returns a stable JSON shape suitable for `toMatchInlineSnapshot()`. Combined with `expectChainIntact` covers the snapshot+diff pattern. |
| X75 | **Fixture generators** | 🔴 | Still missing — no explicit "generate N realistic event rows" helper. `@actantdb/testing` covers the assertion side; the generator side is a one-file follow-up (~120 LOC). |
| X76 | **CI helpers** | 🔴 | Reusable GitHub Action: `Prompt-or-Die-Labs/actantdb-action` that boots a fresh ActantDB server for tests + tears it down. Used as `uses: actantdb/actantdb-action@v1`. |

**Part I: 2 🟢, 2 🔴.**

---

## Part J — Polish (small things that signal "modern")

| # | Item | Status | Notes |
|---|---|--------|-------|
| X77 | **Dark mode on Studio** | 🚧 | CSS uses `prefers-color-scheme`; manual toggle missing. |
| X78 | **Studio i18n** | 🔴 | English only. Defer until first international ask. |
| X79 | **Studio mobile / responsive layout** | 🔴 | Designed for laptop screens. Tablets / phones probably broken. |
| X80 | **Auto-update notifier** | 🔴 | `actantdb` CLI checks for a newer version on `latest` and prints a one-line "your local CLI is behind latest" once per day. |
| X81 | **`actantdb upgrade`** | 🟢 | `crates/actant-cli/src/cmd/upgrade.rs` — `--check` consults `npm view @actantdb/all version` (falls back to the npmjs registry HTTP API) and compares to the running binary's `CARGO_PKG_VERSION`; bare form prints the `npm install -g @actantdb/studio@latest` instruction (the Studio package bundles the actantdb binary entrypoint). |
| X82 | **Homebrew formula** | 🔴 | `brew install actantdb` works for Mac users. ~30 lines of Ruby + tap setup. |
| X83 | **Scoop / Chocolatey** | 🔴 | Same for Windows users. |
| X84 | **APT / RPM repo** | 🔴 | Same for Linux server users. |
| X85 | **NixOS package** | 🔴 | Same for Nix users (a vocal niche). |
| X86 | **Source-mapped npm packages** | 🚧 | We ship `.js.map` from `tsc`; verify they actually resolve correctly when consumers debug. |
| X87 | **`SECURITY.md`** | 🟢 | `SECURITY.md` at repo root: `security@actantdb.dev`, 90-day coordinated-disclosure SLA, in/out-of-scope matrix, PGP fingerprint placeholder, no-bounty note. |

**Part J: 2 🚧, 9 🔴.**

---

## Overall tally

| Status | Count | Notes |
|---|---:|---|
| 🟢 ships | **38** | First-touch, high-volume TS adapters, CLI tooling, MCP, recipes, testing helpers, docs/API references, and the shipped Part K server features. |
| 🚧 partial | **10** | Things that exist but need wrapper hardening, runtime validation, or product polish |
| 🔴 missing | **45** | DX backlog after this pass: language SDKs (Go/Kotlin/.NET/Ruby/PHP/Elixir), edge runtimes (CF Workers/Deno/Vercel Edge), VSCode extension, package managers (Homebrew/Scoop/APT/Nix), trace-UI integrations, big-ticket UI, Studio i18n/mobile/dark-mode toggle, Python framework adapters, durable workflow middleware, gateway metadata capture, and fixture generators. |
| ⊝ deliberate non-goal | **1** | Browser runtime remains a non-goal for the current embedded Node package; a separate WASM package is tracked as X92. |
| **Total rows** | **94** | |

That's a lot of red. **It's not all equal weight.** The high-leverage subset:

## Top 10 high-leverage items (do these first)

In strict ROI order. Each one moves the needle on "would I adopt this?" for
a specific named user persona.

1. **X63 `@actantdb/mcp-server`** — instantly makes us the answer to
   "remember what my agent did" for every Cursor / Claude Desktop user.
   Single npm package, one MCP server impl. Largest leverage per LOC in
   the whole document.
2. **X10 `@actantdb/ai-sdk`** (Vercel AI SDK adapter) — the largest TS
   agent surface; every Next.js AI app uses this.
3. **X11 `@actantdb/openai-agents`** — OpenAI Agents SDK is brand-new
   (2026) and pulling usage fast.
4. **X1 `npm create actantdb`** — table-stakes onboarding; bullet point
   on the homepage.
5. **X3 `actantdb doctor`** — kills 80% of "didn't work" support load.
6. **X12 + X13 `@actantdb/anthropic` and `@actantdb/openai`** — the
   direct-SDK intercepts. Drop-in replacement, zero learning curve.
7. **X32 `actantdb tail`** — the "log-flow" tool every dev expects.
   60 LOC of CLI.
8. **X66/X67 Recipes + typedoc** — discoverability of patterns.
9. **X46 Go SDK** — second-largest substrate language; ops teams default
   to it.
10. **X73 `@actantdb/testing`** — every consumer writes assertion code
    today; ship a library.

If we ship just those 10, the "first 5 minutes" UX goes from "works if
you read the README carefully" to "obviously the right tool".

## Part K — Big-ticket features previously framed as "anti-scope"

Reclassified: every item below is something we ARE building (or want to
plan for), not a deliberate non-goal. They're called out separately
because each is a multi-week effort with its own architecture story.

| # | Item | Status | Notes |
|---|---|--------|-------|
| X88 | **Auto-generated REST API from schema (`@actantdb/auto-rest`)** | 🟢 | `crates/actant-server/src/{auto_rest.rs, schema_introspect.rs}`. PostgREST-style `/rest/v1/<table>` with `select=`/`order=`/`limit=`/`offset=` + filter operators (`eq.`, `neq.`, `lt.`, `gt.`, `lte.`, `gte.`, `like.`, `in.(...)`, `is.null`). `agent_event` + `command_record` stay append-only-via-commands. Feature-gated behind `auto-rest`. Tests in `crates/actant-server/tests/auto_rest_*.rs`. |
| X89 | **GraphQL endpoint** | 🟢 | `crates/actant-server/src/graphql_api.rs` via `async-graphql`. Schema auto-derived from the same introspection that backs X88. Reads = `query`; mutations route through the typed-command envelope (no Hasura-style auto-mutations). Feature-gated behind `graphql`. Tests in `crates/actant-server/tests/graphql_*.rs`. |
| X90 | **Vector database as a primary product surface** | 🚧 | `actant-index` + `actant-embed` substrate already exists. What's missing: first-class API (`box.vectors.upsert/search/delete`), Studio panel, collection lifecycle, hybrid search (vector + metadata), per-collection embedding model. Effort: ~4 weeks. Bumps us into Pinecone / Weaviate / Qdrant comparison set. |
| X91 | **Visual workflow canvas in Studio** | 🔴 | Drag-drop DAG builder that emits `actant-flow::Workflow` definitions. React Flow under the hood. Round-trips: edit in canvas → save → file commit; edit file → reload canvas. Effort: ~4 weeks (a panel-shaped React app on top of an existing Workflow API). |
| X92 | **Browser embedded mode (`@actantdb/core-wasm`)** | 🔴 | WASM SQLite (sql.js or wa-sqlite) so the ledger runs fully client-side. Same API as `@actantdb/core`. Use cases: in-browser agent demos, offline mobile (iOS Safari), zero-backend prototypes. Effort: ~3 weeks; file persistence story is the tricky part (IndexedDB OPFS). |
| X93 | **Generic pub/sub broker mode** | 🟢 | `crates/actant-subscribe/src/broker.rs` + `crates/actant-server/src/pubsub_routes.rs`. Named-topic broker with workspace isolation; persistence via the new `pubsub_message` table (`migrations/0006_pubsub.sql` + `migrations/pg/0006_pubsub.sql` — keeps GAPS row #22 parity gate at 91/91). WebSocket transport at `/v1/pubsub/<workspace>/<topic>`. Five tests in `crates/actant-subscribe/tests/broker_*.rs`. |
| X94 | **Mailpit-equivalent local SMTP catcher** | 🟢 | `deploy/docker-compose.yml` ships Mailpit alongside `actantdb-server` (SMTP on :1025, web UI on :8025); `ACTANTDB_SMTP_HOST`/`ACTANTDB_SMTP_PORT` env wired so any worker that sends mail hits the catcher by default. In-process catcher (no Docker required) is the deferred extension. |
| X95 | **No-code agent builder (full Zapier-shape)** | 🔴 | Tying X91 (workflow canvas) + the agent harness (`@actantdb/box`) + tool definitions into a single drag-drop UI for non-developers. Bigger lift than X91 alone — needs auth, sharing, marketplace. Effort: ~8 weeks. |

**Part K totals:** 4 🟢 (X88 auto-REST, X89 GraphQL, X93 pub/sub broker, X94 Mailpit), 1 🚧 (X90 vector DB), 3 🔴 (X91 workflow canvas, X92 browser embedded, X95 no-code builder). Every row is real work; none of it is deliberately omitted.

## Cross-link audit

| Doc | Scope | Does NOT cover |
|---|---|---|
| [`GAPS.md`](./GAPS.md) | Self-host substrate + BaaS-parity bar | Cloud, DX |
| [`CLOUD_GAPS.md`](./CLOUD_GAPS.md) | Hosted product surface | Self-host, DX |
| **[`DEVX_GAPS.md`](./DEVX_GAPS.md)** | **Local-deployment DX for agent devs** | **Substrate, cloud, business** |
| [`docs/CLOUD_ROADMAP.md`](./docs/CLOUD_ROADMAP.md) | Cloud phasing narrative | — |
| [`COMPARISON.md`](./COMPARISON.md) | Competitive landscape | — |
| [`BENCHMARKS.md`](./BENCHMARKS.md) | Perf numbers | — |
| [`TESTING.md`](./TESTING.md) | Test coverage | — |
