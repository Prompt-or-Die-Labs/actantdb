# ActantDB vs the 2026 agent-tooling market

**Status:** Honest competitive snapshot for ActantDB **0.0.15**.
**Date:** 2026-05-18.
**Authoring rule:** Every competitor claim in this document was checked against
the vendor's own 2026 docs (links at the bottom). Where the docs were silent or
ambiguous, the cell says "unclear" rather than guessing. Where a competitor is
stronger, that's stated plainly.

---

## TL;DR

The 2026 agent-tooling market is **not short of good frameworks** (Mastra,
LangGraph, OpenAI Agents SDK, CrewAI), **good durable backends** (Temporal,
Inngest, Restate, DBOS), **good trace UIs** (Langfuse, LangSmith, Phoenix,
Helicone), or **good memory stores** (Mem0, Zep). What it *is* short of is a
**single accountability layer** that:

1. Records what the model saw + what it requested + who approved + what
   happened, in an **append-only hash-chained ledger** (tamper-evident, not
   just persistent).
2. Runs **Guard verdicts at runtime** against the policy snapshot in force at
   the time of the tool call, and writes that verdict to the ledger as a
   first-class event.
3. Bounds context with a **capsule sensitivity ceiling** that a context
   firewall enforces *before the model call*, not after.

ActantDB ships those three as the substrate, behind a single contract crate that
codegens TypeScript / Python / Swift / Rust SDKs. The rest of the stack
(workflows, memory candidate→approval→use, hybrid retrieval, OTel emission,
multi-tenant, replay) exists so you don't have to wire those three across
five vendors.

If you already pay Temporal + Langfuse + Mem0 + a guardrail library and you're
happy, you do not need ActantDB. If you want the accountability layer as one
thing, and you can accept pre-1.0, that's the trade.

---

## The competitive set (four categories)

### 1. Durable-workflow backends

| Vendor | What it is | What it does that overlaps | License | Default install |
|---|---|---|---|---|
| **Temporal** | The category-defining durable workflow engine. Event history, deterministic replay, versioning via patching. | Records every workflow event; replay reconstructs state. Activity results cached in history. | Apache 2.0 (server + SDKs) | `npm i @temporalio/client @temporalio/worker` |
| **Inngest** | Event-driven durable execution; serverless-first; step functions. | Step-level retries, flow control (throttle/rate limit/batch), 90-day trace retention on Enterprise. | SSPL today, **converts to Apache 2.0 after 3 years** (DOSP). Self-host since 1.0 (Sep 2024). | `npm i inngest` |
| **Restate** | Lightweight durable runtime; journal-based; strong consistency. | Persists handler progress, retries skip completed steps. SDKs in TS/Java/Kotlin/Python/Go/Rust. | **BUSL 1.1** (no "Public Restate Platform Service") | `npm i -g @restatedev/restate-server @restatedev/restate` |
| **DBOS** | Library-based durable workflows backed by Postgres. Came out of MIT/Stanford. | Checkpoints workflow + step state to a system DB; resumes from last completed step. | MIT (Transact); Conductor has a separate license. | `npx @dbos-inc/create@latest -t dbos-node-starter` |

**Overlap with ActantDB:** All four record what happened and resume on failure.
**Gap:** None of them ship a **runtime Guard verdict** (the policy that was in
force, who approved, why), **governed memory lifecycle**, or
**hash-chained tamper-evidence** on the event log. Temporal's history is
ordered and persisted; it is not chained against the prior event's hash. You
cannot prove to an external auditor that the history hasn't been edited.

### 2. Agent frameworks

| Vendor | What it is | What it does that overlaps | License | Default install |
|---|---|---|---|---|
| **Mastra** | TypeScript-first agent framework from the Gatsby team. Workflows, memory, tools, voice, deployers. | Mastra Platform offers hosted runs + traces; framework is full-stack but pluggable. | Apache 2.0 (framework); enterprise features (RBAC/SSO/ACL) need commercial license. | `npm create mastra@latest` |
| **LangGraph** | Low-level graph runtime for stateful agents from LangChain. Checkpointing + fork-from-checkpoint via `get_state_history` + `update_state`. | **Has real time-travel:** thread + `checkpoint_id` lets you fork at arbitrary points to explore alternatives. | MIT (OSS); LangGraph Platform has tiered pricing. | `pip install -U langgraph` |
| **OpenAI Agents SDK** | The successor to Swarm. Agents, handoffs, input/output guardrails, sessions, built-in tracing. | Tracing + guardrails + session memory in one OSS package. | MIT (Apache-compatible upstream) | `pip install openai-agents` |
| **CrewAI** | Multi-agent crews + flows. AMP (SaaS) and Factory (self-host containers). | Built-in memory/knowledge/observability + a deployment console. | Open-core; Enterprise via app.crewai.com | `uv tool install crewai` |

**Overlap with ActantDB:** All four orchestrate agent runs and can capture
traces. LangGraph in particular **does have replay-from-checkpoint and
forking** — this is not an ActantDB-only capability.

**Gap:** None ship a hash-chained ledger; none gate context against a
capsule-bound sensitivity ceiling before model dispatch; none expose a
typed Guard verdict that the policy author wrote (the closest is OpenAI Agents'
guardrails, which validate input/output text but do not snapshot a policy
version into a ledger event).

**ActantDB and these frameworks are not pure competitors.** `@actantdb/mastra`
wraps a Mastra (or LangGraph, or hand-rolled) agent with `withActant()`. The
intended deployment is "your favorite framework + ActantDB underneath."

### 3. Observability / trace platforms

| Vendor | What it is | What it does that overlaps | License | Default install |
|---|---|---|---|---|
| **Langfuse** | Open-source LLM engineering platform: tracing, prompts, evals, datasets, LLM-as-judge. | Captures and visualises every LLM call; prompt versioning; eval pipelines. | MIT core; Enterprise SKU. Self-host free via Docker Compose / k8s. | `npm i langfuse` |
| **LangSmith** | LangChain's hosted observability + eval suite. Trace, eval, prompt hub; deploys agents as Agent Servers. | Trace replay (UI-side), prompt regression, dataset-driven evals. SOC2/HIPAA/GDPR. | Proprietary SaaS; self-host + hybrid in Enterprise. | `pip install langsmith` |
| **Arize Phoenix** | OSS LLM/agent observability built on OpenInference (the de-facto OTel semantic conventions for LLM spans). | Traces, evals, datasets, experiments. Docker/k8s self-host. | **Elastic License 2.0** (no hosted-as-SaaS resale). Self-host free. | `pip install arize-phoenix` |
| **Helicone** | OpenAI-compatible gateway + observability. One-line proxy install. | Logs every request, caching, prompts, evals. | Apache 2.0; self-host via Docker Compose. | Helm chart for Enterprise. | proxy URL swap |

**Overlap with ActantDB:** All four record traces. ActantDB emits OTel + OpenInference spans and is explicitly designed to **export to Phoenix / Arize / LangSmith / Datadog / Grafana / Honeycomb** — `actant-trace` is a span producer.

**Gap (and the honest framing):** Phoenix/Langfuse/LangSmith/Helicone are **trace visualisers**. ActantDB is a **trace producer + event ledger + workflow runner + memory + guard**. These overlap on "the trace UI" but not on the rest. **Pair ActantDB with Phoenix or Langfuse if you want a nicer trace UI than `@actantdb/studio` currently ships** — Studio renders a React-based timeline + replay diff (GAPS.md item 6). Phoenix's trace UI is more mature.

### 4. Memory / accountability

| Vendor | What it is | What it does that overlaps | License | Default install |
|---|---|---|---|---|
| **Mem0** | Self-improving memory layer for LLM apps. Vector + graph + fact extraction. Used by many agent stacks. | Long-term per-user memory, retrieval, conflict resolution. OSS or hosted. | Apache 2.0 (OSS); Platform tier (closed). | `pip install mem0ai` |
| **Zep** | Context-engineering platform + Graphiti temporal knowledge graph (OSS). Episode-credit billing. | Per-session + long-term memory, Graph RAG, context assembly. | Graphiti OSS; Zep Cloud commercial. | SDK install per language. |
| **mem.io** | Newer entrant in agent memory; positioning still consolidating. | Not enough public 2026 doc surface to score meaningfully. | Unclear | Unclear |

**Overlap with ActantDB:** Both Mem0 and Zep store, retrieve, and (in Mem0's case) reconcile agent memories.

**Gap:** Neither runs the **candidate → approval → use** lifecycle as first-class ledger events. Neither blocks a tool call because the *memory that motivated it* exceeded the capsule's sensitivity ceiling. Neither lets you replay a planner with a specific memory **removed** and see what the model would have done.

If you want a **best-in-class pure memory layer**, Mem0 is more mature than `actant-memory` today. ActantDB's memory is governance-first, not retrieval-quality-first.

---

## Feature matrix

Cells use precise text where possible. ✅ = first-class, native. ◑ = partial / via plugin. ✗ = not in product. Where a competitor has a *different but comparable* feature, the cell describes it instead of using a symbol.

| Capability | **ActantDB 0.0.15** | Temporal | Inngest | LangGraph | Mastra | Langfuse | Zep |
|---|---|---|---|---|---|---|---|
| **Durable workflow execution** | ✅ runner + cron + approval-pause | ✅ category leader | ✅ step functions | ✅ checkpointed graph | ◑ workflows, less mature | ✗ (trace UI) | ✗ (memory) |
| **Replay semantics** | 4 typed modes (recorded / model / policy / memory), 3 deferred (`tool`, `experimental`, `local_only`) | Deterministic replay-from-start of event history | Step retry; no decision-point fork | **Fork from any checkpoint via `get_state_history` + `update_state`** | Workflow snapshots | Trace re-render only | ✗ |
| **Hash-chained tamper-evident ledger** | ✅ `chain_hash` per event | ✗ ordered but not chained | ✗ | ✗ checkpoints are mutable rows | ✗ | ✗ | ✗ |
| **Runtime Guard verdict at tool call** | ✅ policy snapshot + verdict are ledger events | ✗ (workflow code can decide) | ✗ | ✗ (HITL exists; no policy snapshot) | ◑ via custom code | ✗ | ✗ |
| **Capsule-bound context w/ sensitivity ceiling** | ✅ `actant-context` enforces before model dispatch | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Governed memory (candidate→approval→use)** | ✅ `actant-memory` lifecycle + conflict detection | ✗ | ✗ | ◑ store API (no approval lifecycle) | ◑ memory primitives | ✗ | ◑ extraction quality, no approval flow |
| **Hybrid retrieval built-in** | ✅ dense cosine + reranker hooks | ✗ | ✗ | ◑ via integrations | ✅ | ✗ | ✅ |
| **OTel + OpenInference traces** | ✅ exports to Phoenix / Arize / Langfuse | ◑ via SDK plugins | ◑ | ◑ via LangSmith | ✅ | **✅ ingests** | ◑ |
| **Multi-tenant w/ cross-tenant guards** | ✅ `actant-tenant` + role checks | Cloud/Enterprise: namespaces | Enterprise plan | LG Platform Enterprise | Mastra Platform enterprise | Pro+/Enterprise | Project-scoped |
| **Contract-driven multi-lang SDKs** | ✅ Rust contract crate → TS/Python/Swift/Rust codegen | Hand-written SDKs per lang | Hand-written SDKs per lang | Hand-written | TS only | TS/Python/Go | TS/Python/Go |
| **License** | Apache 2.0 | Apache 2.0 | SSPL → Apache 2.0 (3y DOSP) | MIT (LG Platform tiers) | Apache 2.0 (commercial EE) | MIT + EE | Graphiti OSS; Zep proprietary |
| **Production storage backend** | SQLite ✅, Postgres ◑ (storage layer yes, command engine still hardcodes SQLite — GAPS.md #5) | Cassandra / Postgres / MySQL | Postgres / SQLite | Postgres / SQLite via checkpointers | Multiple | Postgres + Clickhouse | Postgres + graph |
| **Maturity / production users** | Pre-1.0, no public adopters yet, 429 tests | Netflix, Stripe, Snap, Coinbase | DoorDash, Vercel customers, etc. | LangChain ecosystem at scale | Growing; v1 in 2026 | Hundreds of teams, YC + public | Production agents at scale |

The "Maturity" row is the row that matters most for an honest read of ActantDB
right now. The other rows describe what ActantDB does *better*. This row
describes the price.

---

## Three places ActantDB is genuinely differentiated

These are the claims that survive a code-level audit. Not "we also have X" —
things no competitor in the set does in their 2026 docs.

### 1. Hash-chained append-only event ledger

Every event written to `@actantdb/core` carries a `chain_hash` linking it to
the prior event. Tampering with a past event invalidates the chain forward.
Temporal's event history is ordered and durable; it is not chained against
prior event hashes. LangGraph checkpoints are rows in Postgres or SQLite,
mutable by anyone with DB access. DBOS, Inngest, Restate, all the trace
visualisers — none chain.

**Why it matters:** In regulated workspaces (healthcare, finance), "we have
the logs" is not the same as "we can prove the logs weren't edited." ActantDB
ledgers are tamper-evident by construction; the audit-export pipeline
(`actant-audit-export`) emits the chain so an external auditor can verify
without trusting the database.

### 2. Runtime Guard verdict as a first-class ledger event

When a tool call is requested, `actant-policy` evaluates the current policy
snapshot and writes a `tool_call_approved` / `tool_call_denied` event that
records: which policy version was in force, what scopes were granted, what was
constrained, who approved (human or rule), and why. On replay, that verdict
is reused; in `policy` replay mode, you swap the policy and re-evaluate.

OpenAI Agents SDK has *guardrails* (input/output validation). Mastra has
HITL hooks. LangGraph has interrupt. **None of them snapshot the policy
version into the ledger as a typed event** — meaning none of them can answer
"what would the agent have done if the policy in force at 14:32 yesterday had
been v3 instead of v2?" without re-running the entire workflow.

### 3. Capsule-bound context with sensitivity ceiling enforced before model call

`actant-context` assembles a **context manifest** for each model call — every
retrieved memory, every tool description, every system prompt fragment — and
checks each piece against the capsule's sensitivity ceiling. `Secret`-class
content cannot be sent to a cloud-route model; the check fires *before*
dispatch, and the block is logged as a ledger event with `blocked_reason`.

Mem0 and Zep govern *memory* — they decide what to remember and what to
return. Langfuse and Phoenix observe *afterwards*. **No competitor in this
set governs the context manifest itself before it reaches the model.** This
is the difference between "we redact PII from the trace" (everyone) and "the
model never saw the PII to begin with" (ActantDB).

---

## Places competitors are genuinely stronger

The point of writing this honestly is that the doc is useful when ActantDB is
the right tool and useful when it isn't. Here are the rows where ActantDB
loses today.

- **Temporal at scale.** Temporal runs production workflows at Netflix, Stripe,
  Snap, Coinbase. It has a decade of war stories, a hardened deterministic
  replay model, and patch-based versioning. ActantDB has 429 passing tests
  and zero external production users. **If your bottleneck is "we need
  millions of long-running workflows to not fall over today," use Temporal.**
- **LangGraph for graph-shaped agents + fork experimentation.** LangGraph's
  `get_state_history` + `update_state` + thread-scoped checkpoint forking is
  a clean, well-documented API. ActantDB's policy/model/memory replay is a
  *different* thing, but for the specific "fork the graph and try an
  alternative trajectory" workflow, LangGraph is more polished.
- **Phoenix and Langfuse for the trace UI.** Phoenix ships OpenInference
  (which everyone else implements) and a polished trace explorer. Langfuse
  has a richer prompt-management and eval surface than `@actantdb/studio`.
  Studio shipped a React + Vite UI in 0.0.10 (per `packages/actant-studio/ui-src/`).
  #6). **Plan to use ActantDB as the ledger producer and Phoenix/Langfuse as
  the visualiser** until Studio matures.
- **Mem0 / Zep for memory recall quality.** Mem0 has done serious work on
  fact extraction and conflict resolution at scale; Zep has Graphiti
  (temporal knowledge graphs) and shipping production deployments.
  `actant-memory` is governance-first, not recall-quality-first. If your
  win condition is "the memory layer should be smart," not "the memory
  layer should be auditable," start with Mem0 or Zep.
- **CrewAI / Mastra for time-to-prototype.** `npm create mastra@latest`
  generates a runnable agent in 30 seconds. `withActant()` is one wrapper
  on top of that; it doesn't make Mastra obsolete.
- **Inngest for event-triggered serverless.** If your agent runs in response
  to webhooks, queues, or schedules on Vercel/Netlify/Cloudflare, Inngest's
  serverless story is more mature than `actant-server` for that deploy shape.
- **Helicone for "one line of code".** Helicone is a base-URL swap; logging
  starts instantly. ActantDB is a wrapper + a ledger; the install is heavier.
  If you want LLM call logging *only* and nothing else, Helicone wins on
  ergonomics.

---

## When to use each

Reach for **ActantDB** when:

- You need an **auditable record** of *why* an agent did what it did — model
  inputs, policy snapshot, approver identity, memory provenance — not just
  *what* it did.
- You're shipping to a **regulated workspace** (HIPAA, SOC2, finance,
  healthcare) and "we have the logs" is not sufficient; you need
  tamper-evidence and a sensitivity-ceiling firewall on context.
- You want **workflow + policy + memory + ledger + replay** behind one
  contract crate, with one CLI and one SDK surface, instead of integrating
  Temporal + Langfuse + Mem0 + a guardrail library.
- You're building **local-first agents** that should run entirely on-device:
  `@actantdb/core` is embedded SQLite via `node:sqlite`, no daemon required.
- You can accept **pre-1.0** software in exchange for the unified surface.

Reach for **Temporal** when:

- Your bottleneck is durable workflow execution **at large scale today**,
  with patched versioning and decade-proven failure modes.
- You don't need accountability/memory/context-gating from the same vendor.

Reach for **Inngest** when:

- Your agent is event-triggered (webhooks, queues, schedules) and you deploy
  serverless (Vercel/Netlify/Cloudflare).
- You want step functions with flow control built in.

Reach for **Restate** when:

- You want a lightweight self-hosted durable runtime with strong consistency
  and you're comfortable with BUSL 1.1's terms (no "Public Restate Platform
  Service" — fine for almost all end users).

Reach for **DBOS** when:

- Your stack is Postgres-centric and you want durable workflows as a library
  annotation on existing code, MIT-licensed.

Reach for **Mastra** when:

- You want a **TypeScript-first agent framework** with batteries included
  and you're shipping a product. Pair with `@actantdb/mastra` if you want
  the accountability layer underneath.

Reach for **LangGraph** when:

- You're already in the LangChain ecosystem.
- You specifically need **graph-shaped agents with thread-scoped fork
  exploration** as a core workflow (LangGraph does this better than anyone).
- You're willing to pay LangGraph Platform pricing for hosted deployment, or
  to self-host the OSS runtime.

Reach for **OpenAI Agents SDK** when:

- You're OpenAI-API-only, you want guardrails + handoffs + sessions in a
  lightweight package, and you don't need durable execution outside the
  process.

Reach for **CrewAI** when:

- Your problem shape is "multiple specialised agents collaborating" and you
  want crews/flows as the first-class abstraction.

Reach for **Langfuse** when:

- You want the **best self-hosted trace + eval + prompt management UI**, and
  you'll send spans to it from your existing stack. **Pair with ActantDB**:
  ActantDB exports OTel/OpenInference, Langfuse ingests them.

Reach for **LangSmith** when:

- You're already on LangChain and you want hosted observability + evals with
  SOC2/HIPAA/GDPR compliance out of the box.

Reach for **Arize Phoenix** when:

- You want OpenInference-native traces, you self-host (ELv2 prohibits
  reselling as a hosted service), and you want a polished trace explorer.
  **Pair with ActantDB**.

Reach for **Helicone** when:

- You want the absolute lowest-friction LLM call logging — a base URL swap.

Reach for **Mem0** when:

- Your win condition is **memory recall quality** (fact extraction,
  conflict resolution, multi-user memory) and you don't need a hash-chained
  ledger.

Reach for **Zep** when:

- You want a **temporal knowledge graph** (Graphiti) as the memory substrate.

---

## Pricing snapshot (sources current as of May 2026)

| Vendor | Free / OSS | Paid floor | Notes |
|---|---|---|---|
| **ActantDB** | Apache 2.0 OSS; npm install free | n/a (no commercial SKU yet) | Pre-1.0, no hosted service. |
| **Temporal** | Self-host free (Apache 2.0) | Cloud Essentials **$100/mo** (1M actions, 1 GB active) | Pay-as-you-go actions + storage; $1000 trial credit. |
| **Inngest** | Hobby free (50k execs/mo); self-host free since 1.0 | Pro **$75/mo** (1M execs) | Enterprise custom. License: SSPL → Apache 2.0 after 3y. |
| **Restate** | Self-host (BUSL 1.1, free for end users) | Restate Cloud (custom) | License flips to OSI-compatible after change date. |
| **DBOS** | Transact MIT OSS | DBOS Pro / Teams / Cloud (custom) | Cloud bills compute + DB. |
| **Mastra** | Apache 2.0 framework | Mastra Platform "free to start"; pricing TBD Q1 2026 | EE features (RBAC/SSO/ACL) need commercial license. |
| **LangGraph** | OSS MIT | Developer tier (100k nodes/mo free) → Plus **$49/mo** → Pro **$99/mo** → Enterprise custom. Self-host only on Enterprise. | Plus/Pro store data in GCP US/EU. |
| **OpenAI Agents SDK** | MIT, free | n/a (you pay for model + tracing through OpenAI) | Tracing is part of the platform. |
| **CrewAI** | OSS framework | AMP (SaaS) + Factory (self-host) — pricing on app.crewai.com | |
| **Langfuse** | Hobby free (50k units, 30d retention); self-host free | Core **$29** → Pro **$199** → Enterprise **$2,499**/mo | Pro adds 3-year retention; Enterprise adds SCIM, SLA. |
| **LangSmith** | Free trial | Plus, Enterprise (custom) | SOC2/HIPAA/GDPR; hybrid self-host on Enterprise. |
| **Arize Phoenix** | Self-host free (ELv2); AX Free hosted $0 | AX Pro **$50/mo/user** → up to $1000/mo | ELv2 prohibits reselling as a hosted service. |
| **Helicone** | Hobby (10k req/mo, 7d logs) | Pro **$79/mo** → Team **$799/mo** → Enterprise custom | Apache 2.0; self-host via Docker. |
| **Mem0** | OSS Apache 2.0 | Platform (custom — see app.mem0.ai) | Platform adds dashboards + zero-ops. |
| **Zep** | Free tier (1k credits/mo) | Flex **$125/mo** → Flex Plus **$375/mo** → Enterprise custom | Credit = 350 bytes of episode. |

---

## Install-path comparison (the "one line" test)

The first README line is the truth about a tool's positioning.

```bash
# ActantDB:
npm install @actantdb/mastra
# (no daemon, no Docker, no Rust toolchain in the default path)

# Temporal:
npm install @temporalio/client @temporalio/worker
# (also: run a Temporal Service locally or use Cloud)

# Inngest:
npm install inngest
# (also: run inngest-cli dev for local)

# Restate:
npm install -g @restatedev/restate-server @restatedev/restate

# DBOS:
npx @dbos-inc/create@latest -t dbos-node-starter
# (requires Postgres)

# Mastra:
npm create mastra@latest

# LangGraph:
pip install -U langgraph
# (or: npm install @langchain/langgraph)

# OpenAI Agents SDK:
pip install openai-agents

# CrewAI:
uv tool install crewai

# Langfuse:
npm install langfuse
# (server: docker compose up)

# LangSmith:
pip install langsmith
# (account at smith.langchain.com)

# Phoenix:
pip install arize-phoenix

# Helicone:
# base URL swap (no install)

# Mem0:
pip install mem0ai

# Zep:
pip install zep-cloud
```

ActantDB's default install path is **TypeScript-native, no Rust toolchain,
no Docker, no exposed ports** — and that's enforced as a binding rule in
`CLAUDE.md` ("rule 3: TS-native default install path"). That matches Mastra,
Inngest, Langfuse, LangGraph (TS variant), and Mem0 (npm too) for friction.
Restate, DBOS, and the Rust crates are heavier installs.

---

## What this comparison does not claim

- ActantDB does **not** claim "first" on durable workflows. Temporal got
  there first and is more battle-tested.
- ActantDB does **not** claim "first" on replay-from-decision-point.
  LangGraph supports thread-scoped fork-from-checkpoint via
  `get_state_history` + `update_state`. ActantDB's contribution is the
  **typed replay modes** (`recorded` / `model` / `policy` / `memory`) and the
  fact that policy and memory are first-class ledger events you can swap on
  replay.
- ActantDB does **not** claim "first" on observability. Phoenix + OpenInference
  are the industry standard; ActantDB emits to that standard.
- ActantDB does **not** claim "best" on memory recall quality. Mem0 and Zep
  have done more work on extraction and conflict resolution at the recall
  layer.
- ActantDB **does** claim "first to unify these as a contract-crate-driven
  surface with hash-chained ledgers, capsule-bound context, and runtime Guard
  verdicts as typed events" — and that's the substrate.

---

## Sources

Verified May 2026.

- **Temporal:** [docs.temporal.io](https://docs.temporal.io/temporal),
  [Pricing](https://temporal.io/pricing),
  [Workflows](https://docs.temporal.io/workflows)
- **Inngest:** [Docs](https://www.inngest.com/docs/),
  [Pricing](https://www.inngest.com/pricing),
  [Self-hosting announcement](https://www.inngest.com/blog/inngest-1-0-announcing-self-hosting-support)
- **Restate:** [Docs](https://docs.restate.dev/),
  [Quickstart](https://docs.restate.dev/get_started/quickstart),
  [LICENSE](https://github.com/restatedev/restate/blob/main/LICENSE)
- **DBOS:** [Docs](https://docs.dbos.dev/),
  [TS Programming Guide](https://docs.dbos.dev/typescript/programming-guide),
  [Pricing](https://www.dbos.dev/dbos-pricing)
- **Mastra:** [Docs](https://mastra.ai/docs),
  [Pricing](https://mastra.ai/pricing),
  [License](https://mastra.ai/docs/community/licensing)
- **LangGraph:** [Overview](https://docs.langchain.com/oss/python/langgraph/overview),
  [Persistence](https://docs.langchain.com/oss/python/langgraph/persistence),
  [Platform Plans](https://docs.langchain.com/langgraph-platform/plans)
- **OpenAI Agents SDK:** [README](https://github.com/openai/openai-agents-python),
  [Python docs](https://openai.github.io/openai-agents-python/)
- **CrewAI:** [Docs](https://docs.crewai.com/),
  [Installation](https://docs.crewai.com/installation)
- **Langfuse:** [Docs](https://langfuse.com/docs),
  [Pricing](https://langfuse.com/pricing)
- **LangSmith:** [Docs](https://docs.langchain.com/langsmith)
- **Arize Phoenix:** [Docs](https://arize.com/docs/phoenix),
  [License](https://arize.com/docs/phoenix/self-hosting/license),
  [Pricing](https://phoenix.arize.com/pricing/)
- **Helicone:** [Docs](https://docs.helicone.ai/),
  [Pricing](https://www.helicone.ai/pricing),
  [GitHub (Apache 2.0)](https://github.com/helicone/helicone)
- **Mem0:** [Docs](https://docs.mem0.ai/),
  [GitHub](https://github.com/mem0ai/mem0)
- **Zep:** [Docs](https://help.getzep.com/),
  [Pricing](https://www.getzep.com/pricing)
