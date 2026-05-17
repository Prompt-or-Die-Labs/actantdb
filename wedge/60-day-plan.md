# 60-day plan

Today is 2026-05-17. Day 60 is 2026-07-16.

## Days 1–7 — freeze, repo prep, customer discovery

- ✅ PIVOT.md written. Old `/specs`, `/crates`, `/planning`, `/agents` get `STATUS.md` markers as v2 roadmap.
- Create `packages/actant-mastra`, `packages/actant-core`, `packages/actant-studio` skeletons (done; see `/packages`).
- Skeleton: `pnpm install` works, `pnpm build` produces empty bundle, `pnpm test` runs an empty suite.
- **Customer discovery.** 15 cold conversations with working agent developers — preferably people currently shipping on Mastra or Convex. One question:
  > "What would have to be true for you to add Actant to your agent next week?"
- Validate with the cold README test (see [`validation-tests.md`](./validation-tests.md)).
- Time-audit Swoosh vs Actant for 2 weeks. If Actant-direct work is under 25 hours/week at the end of the audit, redesign before writing crate #2.

## Days 8–21 — Mastra wrapper MVP

`@actant/mastra` captures the events the demo needs. No more.

- `withActant(agent, opts)` accepts a Mastra agent, returns a wrapped agent.
- Captures: `agent.run.started`, `model.call`, `tool.call.requested`, `tool.call.completed`, `approval.required`, `approval.decision`, `context.manifest.hash`.
- Events written to a local SQLite file under `~/.actant/<project>/events.sqlite` and broadcast on a Unix socket / IPC channel for Studio.
- `actant studio` boots a local HTTP server (Vite dev or built bundle) and renders a timeline of those events.
- No remote backend. No multi-user. No HTTP API. Local-only.

Acceptance: a sample Mastra agent run produces a Studio timeline with model call → tool call → approval → tool result visible.

## Days 22–35 — Guard authority

The runtime gate.

- Verdicts: `allow` | `constrain(input)` | `require_approval` | `block` | `halt`.
- Policy = small TS object. v0.1 supports: per-tool risk class, regex deny-list on arguments, sensitivity ceiling, hardcoded "shell.run requires approval" defaults.
- Policy snapshot recorded with every tool call so replay can re-evaluate under a different policy.
- Approval API: `actant approve <tool_call_id>` from CLI, or click in Studio.
- A constrained variant rewrites arguments before the underlying tool sees them; the rewrite is recorded.

Acceptance: the killer-demo "approve the constrained variant of `rm -rf build`" path works end-to-end.

## Days 36–50 — Replay that actually works

This is the hardest piece. Don't compromise it.

- Replay checkpoint = `(event_id, model context manifest hash, policy hash, memory set hash, prior tool results)`.
- Replay from checkpoint reruns from that event with overrides: alternate policy, exclude one memory, swap one model.
- `actant replay run <event_id> [--policy ...] [--without-memory ...] [--model ...]` produces a replay run.
- Replay does NOT re-execute real side effects. Tool results in replay mode come from recorded results unless the user explicitly opts into experimental mode (deferred from v0.1).
- `actant replay diff <run_a> <run_b>` returns an event-stream diff: identical / changed / missing / extra.

Acceptance: the killer-demo "replay without memory X; replay with stricter policy; diff" path works end-to-end and Studio renders the diff.

## Days 51–60 — install sprint

No more architecture. Installs.

- 10 live install calls with working agent developers. 30 minutes each.
- Goal: 5 of those 10 run `@actant/mastra` on their own agent, capture a real run, open Studio, click replay.
- 3 keep it installed after 72 hours. 1 uses it in production or serious staging.
- Each install produces a fix list. Fixes ship daily.
- Public examples ship: one repo per design partner that includes a `@actant/mastra` wired Mastra agent + screenshots + replay link.

Day-60 deliverables are documented in [`kill-criteria.md`](./kill-criteria.md). If they don't land, stop building the substrate vision and stay plugin-only.

## What we do NOT do in 60 days

See [`anti-scope.md`](./anti-scope.md). The short version: no Rust core, no Convex wrapper unless a design partner asks, no full CLI, no schema DSL, no AI-native indexing, no protocols, no observability layer, no reliability primitives, no hot kernel, no deployment modes, no Studio dashboard beyond the demo timeline, no Swoosh public launch.
