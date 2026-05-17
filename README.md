# Actant

**Your agent just called a tool. Do you know why?**

Actant records what the model saw, what action it requested, who approved it, what happened, and lets you replay the run from any decision point.

Works with Mastra today. Convex next.

---

## Install

```bash
npm install @actantdb/mastra
```

```ts
import { withActant } from "@actantdb/mastra";

export const agent = withActant(myMastraAgent, {
  project: "prod-support-agent",
  replay: true,
  approvals: true,
});
```

```bash
npx actant studio
```

Open the timeline. Click any tool call. See the model context, the approval scope, the tool result. Click **Replay** to rerun the run from before that decision — with or without a memory, with a stricter policy, with a different model.

You keep Mastra. You keep Convex. Actant adds the flight recorder and the authority gate.

---

## The hero diff

What Studio shows when you replay the killer-demo run with `mem_42_dist` excluded and a stricter policy. From [`wedge/killer-demo.md`](./wedge/killer-demo.md):

```
event              original (recorded)            replay (under overrides)
─────────────────────────────────────────────────────────────────────────────
context_build      3 included, 0 blocked          2 included, 1 blocked  ◀ mem_42 blocked
model_call         "rm -rf build dist"            "rm -rf build"
guard_verdict      require_approval (constrain)   allow
tool_call          shell.run "rm -rf build"       shell.run "rm -rf build"
effect_completed   exit=0                         exit=0  (recorded reuse)
```

> Without `mem_42`, the planner would have proposed the safe command directly.
> The memory caused the risky proposal; Guard caught it; replay proves the causal link.

Try it:

```bash
pnpm install
pnpm --filter actant-demo-test-cleanup demo
ACTANTDB_STORE_DIR=./wedge/demo/.actantdb \
  npx actantdb studio --project demo-test-cleanup
```

---

## Why this exists

Production agents fail in ways traces can't explain. A model sees the wrong context, calls the wrong tool, the human approves something they didn't fully understand, the file gets deleted, the customer gets a wrong email. A trace shows what happened. It does not show **why**, and it does not let you rerun the path with one piece changed.

Actant is the missing layer:

- **Guard Authority** — runtime gate on every tool call. Allow / constrain / require approval / block / halt. With policy snapshot and audit evidence.
- **Chronicle Replay** — causal record of model calls, tool calls, context manifests, approvals, and effect results. Replay from any point under a different policy, different memory set, different model.

That's it. Two features. Everything else waits until those have users.

---

## What Actant is *not*

- Not a Mastra replacement. Mastra builds and runs the agent; Actant records and gates it.
- Not a Convex replacement. Convex is your durable backend; Actant adds replay + authority on top.
- Not a vector database, workflow engine, or model router.
- Not a new agent framework, language, or runtime.

If your team picked Mastra, LangGraph, OpenAI Agents SDK, CrewAI, or built your own loop, Actant plugs in. You don't migrate to use it.

---

## The 10-minute test

If `@actantdb/mastra` doesn't work on your real agent inside 10 minutes from this README, it's broken. File an issue. That's the bar.

---

## Status

**v0.1 wedge in development.** First milestone target: 2026-06-30 (`@actantdb/mastra` wrapper + Studio timeline + tool-call approval + context manifest + basic replay checkpoint, with 3 non-Wes developers having run it on a real agent).

Hard gates and the kill criteria for the v0.1 wedge are in [`PIVOT.md`](./PIVOT.md). The 60-day operational plan is in [`wedge/60-day-plan.md`](./wedge/60-day-plan.md). The killer-demo storyboard is in [`wedge/killer-demo.md`](./wedge/killer-demo.md).

If the wedge proves out, the deeper substrate vision (governed event ledger, causal DAG, sensitivity lineage, MCP/A2A/AP2 protocols, hot kernel + async lanes, six deployment modes — 258 files of plan) becomes the v2 roadmap. It's preserved under [`/specs`](./specs), [`/crates`](./crates), [`/planning`](./planning), [`/agents`](./agents) with `STATUS.md` markers. Don't read those first; they're for after.

---

## Repository layout

```
/                                  ← active product
├── PIVOT.md                       — what changed on 2026-05-17 and why
├── README.md                      — you are here
├── wedge/                         — operational plan for v0.1
│   ├── README.md
│   ├── positioning.md
│   ├── 60-day-plan.md
│   ├── killer-demo.md
│   ├── validation-tests.md
│   ├── kill-criteria.md
│   ├── metrics.md
│   ├── distribution-plan.md
│   └── anti-scope.md
├── packages/                      — npm packages (TypeScript first)
│   ├── actant-mastra/             — @actantdb/mastra (wedge entry point)
│   ├── actant-core/               — @actantdb/core (embedded TS runtime; NAPI-RS native + WASM)
│   ├── actant-types/              — @actantdb/types (generated from crates/actant-contracts)
│   ├── actant-policy/             — @actantdb/policy (Guard verdict builders)
│   ├── actant-replay/             — @actantdb/replay (replay engine)
│   ├── actant-studio/             — @actantdb/studio (UI + `actantdb` CLI)
│   └── actant-convex/             — @actantdb/convex (optional, Phase 2)
├── crates/                        — internal Rust (invisible to TS devs)
│   ├── actant-contracts/          — single source of truth for all public types
│   ├── actant-kernel/             — fast Rust implementation
│   ├── actant-napi/               — Node native addon (bundled into @actantdb/core)
│   ├── actant-wasm/               — WASM fallback (bundled into @actantdb/core)
│   └── (the v2 substrate crates remain present but frozen)
├── premortem-report-*.html        — reference: 2026-05-17 premortem
├── premortem-transcript-*.md
│
└── (v2 substrate roadmap; frozen) — DO NOT pick up these work packages
    /specs, /planning, /agents, /migrations, /templates, /examples,
    /crates (v2 portion: actant-storage, actant-command, actant-effects,
    actant-context, actant-memory, actant-flow, actant-replay,
    actant-subscribe, actant-server, actant-cli, actant-sdk-codegen,
    actant-workers-*, actant-embed, actant-embedders, actant-capsule,
    actant-trust, actant-trigger, actant-eval, actant-sync, actant-audit-export,
    actant-templates, actant-schema-dsl, actant-codegen-project,
    actant-prompts, actant-models, actant-cache, actant-trace, actant-protocol,
    actant-throttle, actant-circuit, actant-lock, actant-ingress, actant-policy)
```

---

## Sources / prior art

Mastra's 1.0 announcement, Convex Agents docs and Workflow component, OpenInference + OpenTelemetry GenAI conventions, SpacetimeDB reducers, Temporal durable execution. The premortem at the repo root cites every claim about the May 2026 market landscape.

---

## License

Apache 2.0. See [LICENSE](./LICENSE).
