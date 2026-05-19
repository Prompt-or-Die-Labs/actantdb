# DEVX_GAPS — local-deployment developer experience

The substrate is solid (see [GAPS.md](./GAPS.md) — 25/38 🟢) and the cloud
roadmap is mapped ([CLOUD_GAPS.md](./CLOUD_GAPS.md)). What's *not* yet
explicitly tracked: the **developer experience** of using ActantDB locally,
specifically for **AI-agent devs who already have a stack** and need to drop
us in without rewriting their app.

This file is the audit. Same status taxonomy as `CLOUD_GAPS.md`:
🟢 ships / 🚧 partial / 🔴 missing / ⊝ deliberate non-goal / 👤 human-only.

Cross-reference: [GAPS.md](./GAPS.md), [CLOUD_GAPS.md](./CLOUD_GAPS.md),
[COMPARISON.md](./COMPARISON.md), [TESTING.md](./TESTING.md).

Last updated: 2026-05-19.

---

## Part A — First-touch onboarding (0 → first-event)

The "I just heard about this; can I get something running in 5 minutes" path.
Everything here is table-stakes for 2026; the bar is set by Next.js, Vite,
Astro, Convex, and Supabase.

| # | Gap | Status | Notes |
|---|-----|--------|-------|
| X1 | **`npm create actantdb` / `npx @actantdb/create-app`** | 🔴 | Convex has `npm create convex@latest`. Next.js has `npx create-next-app`. We have `actant-templates::TemplateRegistry::render` (5 templates) + GAPS row #25 (`actantdb init` CLI). Need one *more* level above that: an npm-installable scaffolder that walks the user through template choice, framework choice, and writes the project. This is the bullet point on the homepage. |
| X2 | **First-launch Studio "welcome" screen** | 🔴 | Today Studio opens with an empty timeline if no events exist. Convex / Supabase Studio both show "you have 0 records — here's how to write your first". Need: empty-state in `packages/actant-studio/ui-src/panels/RunsPanel.tsx` with a copy-pasteable snippet that captures one tool call. |
| X3 | **`actantdb doctor`** | 🔴 | Diagnose: Node version (≥ 22.5), disk space, port 4555/54323 already in use, missing `claude`/`codex` CLI on PATH (for `@actantdb/box`), invalid `ACTANTDB_DATABASE_URL`, etc. Prints one-line fixes. Saves 80% of "it didn't work" support load. |
| X4 | **Pretty errors with one-line fixes** | 🚧 | We have `BoxError` with typed codes (see `@actantdb/box`). Many SDK / CLI errors are still raw `Error.message`. Need an `ActantError` base with `code`, `hint`, `fix_command?` shape applied to every public throw. |
| X5 | **Interactive 5-minute tutorial** | 🔴 | Convex's quickstart is a clickable 5-step tutorial. Ours is a static markdown demo. Could be a Stackblitz / Replit / CodeSandbox link from the README that opens with `@actantdb/all` preinstalled. |
| X6 | **CLI shell completion** | 🔴 | bash / zsh / fish completion for `actantdb` subcommands. `clap` ships `clap_complete` — ~30 LOC. |
| X7 | **First-run telemetry opt-in (truthful)** | 🔴 | One prompt on first `actantdb` invocation: "share anonymous usage so we can fix what breaks?" with a clear opt-out path. Convex + Vercel do this. Don't be sneaky; the prompt itself is the trust-builder. |

**Part A: 7 🔴 (none shipped).** All are small in isolation; the cumulative
effect is "ActantDB feels modern" vs "I have to figure it all out".

---

## Part B — Framework adapters (plug into what they're already using)

The user said: *"easily compatible with what users may already be using"*.
Here's the map.

| # | Framework | Status | Notes |
|---|---|--------|-------|
| X8  | **Mastra** | 🟢 | `@actantdb/mastra` ships `withActant()`. The canonical adapter. |
| X9  | **LangGraph** | 🚧 | Demo at `examples/langgraph-router/` works, but there's no `@actantdb/langgraph` dedicated package. Most LangGraph users will look for one by name. Need: `packages/actant-langgraph/` re-exporting `withActant` with LangGraph-idiomatic naming + a `LangGraphNode` wrapper. |
| X10 | **AI SDK by Vercel** | 🔴 | Vercel's `ai` package is the largest TS agent surface today (>1M weekly downloads). Need: `packages/actant-ai-sdk/` that wraps `streamText` / `generateText` / `generateObject` and records every call as a `model_call` event. Tool calls captured automatically because `ai`'s `tools` shape is structured. |
| X11 | **OpenAI Agents SDK** (`@openai/agents`) | 🔴 | OpenAI's new Agents SDK launched 2026; large adoption coming. Need: `packages/actant-openai-agents/` mirror of @actantdb/mastra. |
| X12 | **Anthropic SDK direct** (`@anthropic-ai/sdk`) | 🔴 | Many devs skip the framework layer and call Anthropic directly. Need: `@actantdb/anthropic` intercept proxy — `import Anthropic from "@actantdb/anthropic"` instead of `from "@anthropic-ai/sdk"` and every message gets logged as a `model_call`. Same shape as a regular Anthropic client; zero learning curve. |
| X13 | **OpenAI SDK direct** | 🔴 | Same as X12 for `openai` package. |
| X14 | **CrewAI** (Python) | 🔴 | Python — our SDK exists. Need a `crewai-actantdb` package on PyPI with a `with_actant_logging(crew)` decorator. |
| X15 | **AutoGen** (Microsoft, Python) | 🔴 | Same shape; Python. |
| X16 | **LangChain JS** | 🔴 | LangChain is still huge; `@actantdb/langchain` with `withCallbackHandler` plug-in. |
| X17 | **LangChain Python** | 🔴 | Same; Python pip package. |
| X18 | **Inngest** | 🔴 | Inngest is the canonical durable-workflow alt to QStash. `@actantdb/inngest` middleware that logs every step as a ledger event. |
| X19 | **Trigger.dev** | 🔴 | Same shape as Inngest. |
| X20 | **Vercel AI Gateway** | 🔴 | If a user proxies through Vercel AI Gateway, we should record `model_call` events with gateway routing metadata intact. |
| X21 | **Ollama / local models** | 🚧 | `actant-runtime::models` registry knows about Ollama. The `withActant` wrapper sees the model name. Need explicit guidance in docs + a `examples/ollama-only/` demo. |
| X22 | **Convex** | 🚧 | `@actantdb/convex` exists. Untested; needs a smoke demo showing "wrap a Convex action so its result lands in the ActantDB ledger". |
| X23 | **Supabase** | 🔴 | The opposite direction — adapter for *running ActantDB inside a Supabase Edge Function* so a Supabase consumer can add ActantDB without standing up a separate server. Worth shipping once GAPS row #26 (docker-compose) ships. |

**Part B: 1 🟢, 3 🚧, 12 🔴, 0 ⊝.** Every 🔴 is a one-package implementation
mirroring `@actantdb/mastra`'s 200-line pattern. Highest priority by
download volume: X10 (Vercel AI SDK), X11 (OpenAI Agents), X12+X13 (direct
SDKs), X16 (LangChain JS).

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
| X32 | **`actantdb tail`** | 🔴 | `tail -f` for the ledger. Filter by topic / event-kind / actor. Pretty-print model_call summaries inline. The "log-flow" tool every dev expects. |
| X33 | **`actantdb watch <predicate>`** | 🔴 | Live filter using the new GAPS row #20 predicate language. `actantdb watch 'kind == tool_call_completed AND payload.tool_name == issue_refund'`. |
| X34 | **`actantdb shell` REPL** | 🔴 | Node REPL with `ledger` + `withActant` + `policy` preloaded. Great for poking at a captured run. Same shape as `python manage.py shell` or `rails console`. |
| X35 | **`actantdb explain <event_id>`** | 🔴 | Natural-language explanation of one event row. Walks the upstream chain ("this `tool_call_completed` was triggered by a `tool_call_requested` from agent X, gated by Guard verdict Y"). |
| X36 | **`actantdb sql`** | 🔴 | Read-only SQL prompt against the ledger DB. Many devs prefer SQL to UI for exploration. Auto-completes table names. |
| X37 | **`actantdb export`** | 🔴 | Dump to JSON / NDJSON / CSV / Parquet. For data-warehouse / pandas / R workflows. Honors capsule sensitivity ceiling. |
| X38 | **`actantdb import`** | 🔴 | Bootstrap a ledger from an existing JSON dump. Useful for testing against production-shaped data. |
| X39 | **VSCode extension** | 🔴 | Inline event count next to function declarations. Click a function → see the events it produced. Hover a `withActant`-wrapped call → see the recent verdicts. Big leverage for adoption. |
| X40 | **Cursor / Windsurf rules** | 🔴 | Ship a `.cursorrules` / `windsurf.config.md` snippet that teaches Cursor about our APIs. Tiny effort, big AI-coding-assistant adoption boost. |
| X41 | **Browser DevTools extension** | 🔴 | For inspecting `@actantdb/sdk` WebSocket subscriptions in dev. Network panel already shows them; this would parse + decode. Niche. |

**Part D: 0 🟢 / 10 🔴.** The CLI subcommands (X32–X38) are days of work
each — `clap` subcommands wrapping existing SDK methods. The VSCode
extension is weeks but **extremely high leverage** for adoption among
AI-assisted devs.

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
| X50 | **PHP** | 🔴 | Missing. Laravel ecosystem has growing AI adoption. |
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
| X53 | **`/metrics` Prometheus endpoint** | 🚧 | OTLP includes metrics but no dedicated Prom endpoint on `actant-server`. Add one: ~30 LOC + `prometheus` crate. |
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
| X63 | **`@actantdb/mcp-server`** | 🔴 | Stdio + HTTP MCP server exposing tools like `list_runs`, `get_event(id)`, `replay(event_id, mode, overrides)`, `query_predicate(...)`. One npm package + a small README. **Highest single-leverage item in this whole document** — instantly makes ActantDB the answer to "remember what my agent did" for every Cursor / Claude Desktop user. |
| X64 | **MCP resource discovery** | 🔴 | Expose recent runs as MCP resources (URIs like `actant://workspace/{ws}/session/{sid}`) so agents can subscribe to live state. |
| X65 | **One-click "Add to Claude Desktop"** | 🔴 | A button on the README/website that registers `@actantdb/mcp-server` into the user's `claude_desktop_config.json`. |

**Part G: 0 🟢 / 3 🔴.** X63 is the breakthrough item. Cursor + Claude
Desktop adoption of ActantDB depends on this existing.

---

## Part H — Documentation, recipes, content

Where someone goes to learn a pattern.

| # | Item | Status | Notes |
|---|---|--------|-------|
| X66 | **API reference (typedoc / rustdoc)** | 🚧 | `rustdoc` autogens on the Rust side. `typedoc` is not wired for the npm packages. |
| X67 | **`docs/recipes/`** | 🔴 | ~15 named patterns: "add approval to a tool", "replay last night's failed run", "wire ActantDB into a Next.js app", "use ActantDB with Ollama only (no cloud models)", "test an agent with snapshot fixtures", "export to BigQuery", "share a replay session for code review", "audit-export to S3 on a schedule", etc. |
| X68 | **Video tutorials** | 👤 | Tied to GAPS row #10 (screencast). Pattern library would be a 5–10 min video series. |
| X69 | **"Awesome ActantDB"** list | 🔴 | Curated list of community examples once we have any. Empty for now; can seed with our 3 demos + 5 templates. |
| X70 | **Migration guides FROM other tools** | 🔴 | "Migrating from Langfuse to ActantDB" / "Adding ActantDB on top of your Inngest workflows" / "Replacing in-house logging with the ActantDB ledger". Inbound-marketing gold. |
| X71 | **Interactive playground** (Stackblitz / CodeSandbox) | 🔴 | Embed in the README. Visitor can click "Run" without installing anything. |
| X72 | **Architecture diagrams** | 🚧 | `specs/` has ASCII diagrams. None of them render well on GitHub or docs sites. Need: SVG diagrams committed alongside, or Mermaid blocks. |

**Part H: 2 🚧, 5 🔴, 1 👤.**

---

## Part I — Testing tools for consumers

Things our users need to test *their* agent code that integrates with us.

| # | Item | Status | Notes |
|---|---|--------|-------|
| X73 | **`@actantdb/testing`** | 🔴 | Mock ledger, fixture builders, helpers like `expectEventEmitted("guard_verdict", { decision: "block" })`. Replaces hand-rolled assertions consumers write today. |
| X74 | **Snapshot testing** | 🚧 | The replay engine + diff IS snapshot testing for agents. Need to wrap it in a vitest-shaped API: `expect(run).toMatchReplaySnapshot()`. |
| X75 | **Fixture generators** | 🔴 | "Generate 1000 realistic event rows for load testing". Useful for benchmarking + reproducing bugs. |
| X76 | **CI helpers** | 🔴 | Reusable GitHub Action: `Prompt-or-Die-Labs/actantdb-action` that boots a fresh ActantDB server for tests + tears it down. Used as `uses: actantdb/actantdb-action@v1`. |

**Part I: 1 🚧, 3 🔴.**

---

## Part J — Polish (small things that signal "modern")

| # | Item | Status | Notes |
|---|---|--------|-------|
| X77 | **Dark mode on Studio** | 🚧 | CSS uses `prefers-color-scheme`; manual toggle missing. |
| X78 | **Studio i18n** | 🔴 | English only. Defer until first international ask. |
| X79 | **Studio mobile / responsive layout** | 🔴 | Designed for laptop screens. Tablets / phones probably broken. |
| X80 | **Auto-update notifier** | 🔴 | `actantdb` CLI checks for a newer version on `latest` and prints a one-line "you're on 0.0.13, latest is 0.0.14" once per day. |
| X81 | **`actantdb upgrade`** | 🔴 | One command to pull the latest binary + npm packages. Saves Googling. |
| X82 | **Homebrew formula** | 🔴 | `brew install actantdb` works for Mac users. ~30 lines of Ruby + tap setup. |
| X83 | **Scoop / Chocolatey** | 🔴 | Same for Windows users. |
| X84 | **APT / RPM repo** | 🔴 | Same for Linux server users. |
| X85 | **NixOS package** | 🔴 | Same for Nix users (a vocal niche). |
| X86 | **Source-mapped npm packages** | 🚧 | We ship `.js.map` from `tsc`; verify they actually resolve correctly when consumers debug. |
| X87 | **`SECURITY.md`** | 🔴 | Disclosure address + SLA. Required for SOC2 (CLOUD_GAPS E7); trivial to write today. |

**Part J: 2 🚧, 9 🔴.**

---

## Overall tally

| Status | Count | Notes |
|---|---:|---|
| 🟢 ships | **8** | Mastra adapter, Node runtime, Swift SDK, Rust SDK, TS SDK, OTel exporter, plus partial Mastra/Convex |
| 🚧 partial | **12** | Things that exist but need a wrapper / hardening |
| 🔴 missing | **66** | The actual DX backlog |
| ⊝ deliberate non-goal | **1** | Browser embedded mode |
| 👤 human-only | **1** | Video tutorials |
| **Total rows** | **87** | |

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
   (2026) and pulling adoption fast.
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

## What we should NOT build (anti-scope)

- ⊝ Browser embedded mode (no `node:sqlite`; consumers use `@actantdb/sdk`
  over HTTP).
- ⊝ Visual workflow canvas / no-code IDE (we're TS-first, not Zapier).
- ⊝ Auto-generated REST from schema (agent-event ledger isn't CRUD).
- ⊝ Generic pub/sub broker (subscribe is per-event-kind, not a Kafka
  alternative).
- ⊝ Vector database as primary product (`actant-embed` is substrate for
  retrieval; we don't compete with Pinecone).

These are decisions, not gaps. Documented in `CLOUD_GAPS.md` and here so
nobody opens a PR titled "Visual workflow builder".

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
