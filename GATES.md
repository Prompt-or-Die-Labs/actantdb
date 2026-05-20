# GATES — current status

This file tracks the three validation gates from [PIVOT.md](./PIVOT.md). It is
not a planning document. It is a punch list: what's *done* (artifact-shaped)
and what's *blocked on external human action*.

Source of truth for gate definitions: [PIVOT.md §Hard validation gates](./PIVOT.md).

## Gate 1 — MVP green (target 2026-06-30)

**Acceptance criterion (PIVOT.md):**

> `@actantdb/mastra` wraps a Mastra agent, captures tool calls + context
> manifest, supports approval, opens Studio with timeline + replay.

| Item | Status | Evidence |
| --- | --- | --- |
| `@actantdb/mastra` wraps a Mastra agent | ✅ | [`packages/actant-mastra/src/index.ts`](./packages/actant-mastra/src/index.ts) — duck-typed wrapper accepts any agent with `tools: Record<string, {execute}>`, peer-deps on `@mastra/core` |
| Captures tool calls | ✅ | `tool_call_requested` / `tool_call_started` / `tool_call_completed` events; round-trip tested in [`packages/actant-mastra/src/index.test.ts`](./packages/actant-mastra/src/index.test.ts) |
| Captures context manifest | ✅ | `context_build` event + `buildContextManifest()` in [`packages/actant-core/src/runtime.ts`](./packages/actant-core/src/runtime.ts) |
| Supports approval (allow / constrain / require_approval / block / halt) | ✅ | Five verdicts in [`packages/actant-policy/src/index.ts`](./packages/actant-policy/src/index.ts); constrain rewrite verified by stock-shaped tool in [`scripts/smoke.mjs`](./scripts/smoke.mjs) |
| Opens Studio with timeline + replay | ✅ | `actantdb studio` CLI in [`packages/actant-studio/src/cli.ts`](./packages/actant-studio/src/cli.ts) serves `ui/index.html`; replay form posts to `/api/replay` and renders a side-by-side diff |
| Killer-demo deliverables (killer-demo.md §"Demo deliverables") | partial | Demo scaffold at [`examples/test-cleanup/`](./examples/test-cleanup) with ≤200-word README; the 90-second screen recording and the hero image are human-produced artifacts |
| "≤ 5 min from `git clone`" | ✅ | Empirically measured TS-only path runs in ~3 seconds (warm pnpm store); full path including `cargo run -p actant-contracts -- codegen-ts` measured at ~11 seconds |
| Workspace smoke test passes on every PR | ✅ | `pnpm smoke` invokes [`scripts/smoke.mjs`](./scripts/smoke.mjs), covering: session → message → manifest → tool request → Guard verdict → approval → constrain-rewritten execution → completion → checkpoint → headless Studio render |
| `cargo check --workspace --all-targets` green | ✅ | 39 Cargo workspace members check under Rust 1.88 |
| Per-package vitest + focused Rust tests green | ✅ | `pnpm -r test`; focused local Rust tests listed in [`TESTING.md`](./TESTING.md) |

**Gate 1: implementation-complete.** The human-only piece — the 90-second
recording and the hero PNG — falls under §"Gate 1 leftovers" below.

### Gate 1 leftovers (human-execution)

- [ ] Record a 90-second screencast of `node examples/test-cleanup/demo.mjs` followed by clicking through Studio (anti-scope rule #2 implies this remains in scope).
- [ ] Export the side-by-side diff as a PNG for the README hero (the ASCII version exists in [`README.md`](./README.md)).
- [ ] **Verify on three real non-Wes developers** that the demo runs from `git clone` on their machine inside 5 minutes. (One per platform: macOS, Linux, Windows.)

## Gate 2 — External adoption (target 2026-07-31)

**Acceptance criterion (PIVOT.md):**

> 10 non-Wes developers installed; 5 used on real projects; 3 kept past one
> week; 2 weekly-feedback design partners.

**Status: BLOCKED on external developer engagement.** No artifact closes this
gate. The actions below are what needs to happen, in order. None of them is
something an agent in this repo can perform — they all require Wes or a
collaborator to execute outside the repo.

Pre-conditions that *are* artifact-shaped:

| Pre-condition | Status |
| --- | --- |
| `@actantdb/mastra` installable via `npm install` (TS-only, no Rust prerequisite) | 🟡 latest npm is `0.0.12`; current workspace is `0.0.15` and still needs publish |
| Cold-README test scaffolding (the README a stranger reads) | ✅ root [`README.md`](./README.md) + [`examples/test-cleanup/README.md`](./examples/test-cleanup/README.md) |
| 10-minute install test scaffolding | ✅ `pnpm install` + `pnpm -r build` + `node examples/test-cleanup/demo.mjs` measured at 3s on a warm cache |
| Per-failed-install ticket process | ✅ documented in [`README.md`](./README.md) |
| One-screen positioning artifact | ✅ [`README.md`](./README.md) |

### What humans must do for Gate 2 to close

- [ ] Send the cold-README test to 15 working agent developers (see [`README.md` §1](./README.md)).
- [ ] Run the 10-minute install test with at least 10 developers (see [`README.md` §2](./README.md)).
- [ ] Track: 7/10 install successfully in <10 min, 5/10 capture a real run, 3/10 produce a replay.
- [ ] Identify 2 design partners willing to provide weekly feedback for 4 weeks.
- [ ] Publish the current `0.0.15` `@actantdb/*` packages to npm (confirmation-required action; latest verified npm version is `0.0.12`).

## Gate 3 — Shipped/staged (target 2026-08-17)

**Acceptance criterion (PIVOT.md):**

> 5 non-Wes devs shipped or staged with Actant; 2 public examples; 1 named
> design partner.

**Status: BLOCKED on external developer engagement.** Cannot be closed from
inside the repo.

Pre-conditions that *are* artifact-shaped:

| Pre-condition | Status |
| --- | --- |
| First public example (the killer-demo rehearsal) | ✅ [`examples/test-cleanup/`](./examples/test-cleanup) |
| Second public example | ✅ [`examples/langgraph-router/`](./examples/langgraph-router) |
| Third public example | ✅ [`examples/cli-only/`](./examples/cli-only) |
| HN test answer prepared | ✅ [`README.md`](./README.md) §"HN objection" |
| Switch-test scaffolding (per `validation-tests.md` §3) | ✅ |

### What humans must do for Gate 3 to close

- [ ] Get 5 non-Wes developers to ship or stage Actant in production / staging.
- [ ] Land 1 named design partner (publicly attributable).
- [ ] Convert external usage into 2 public writeups or repos if the design partners allow attribution.

## Honest summary

- **Gate 1** is implementation-complete in the repo. The ≤200-word README, working demo, Studio CLI, replay diff, smoke test, and 5-min install path all exist and are tested.
- **Gates 2 and 3** measure events in the world (current npm publish, installs, sustained use, named partners). They do not close on repo artifact work alone.

If "100% complete" means "every gate's threshold met", the bottleneck is
external adoption. If it means "every artifact prerequisite to the gates is
green", the remaining repo-side release prerequisite is publishing `0.0.15`.
