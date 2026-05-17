# wedge/

Operational plan for the v0.1 product. Read these in order:

1. [`f2-f3-prevention.md`](./f2-f3-prevention.md) — **binding** product constraints (TS-native default, contract-first build). Supersedes anything below it contradicts.
2. [`positioning.md`](./positioning.md) — one-sentence answer, HN objection answer, positioning map.
3. [`60-day-plan.md`](./60-day-plan.md) — week-by-week execution.
4. [`killer-demo.md`](./killer-demo.md) — the demo that defines the wedge.
5. [`validation-tests.md`](./validation-tests.md) — cold README, install, switch tests. Pass/fail thresholds.
6. [`kill-criteria.md`](./kill-criteria.md) — three hard gates (Jun 30, Jul 31, Aug 17) with named actions on miss.
7. [`metrics.md`](./metrics.md) — completed external replays is the one metric. Stars don't count.
8. [`distribution-plan.md`](./distribution-plan.md) — Mastra ecosystem first, Convex second, cross-framework later.
9. [`anti-scope.md`](./anti-scope.md) — the binding list of what NOT to build before the wedge proves out.

## The wedge in 30 seconds

Two features:

- **Guard Authority** — runtime gate on every tool call. Allow / constrain / require approval / block / halt. Policy snapshot + audit evidence.
- **Chronicle Replay** — model call + tool call + context manifest + approval + effect result, all causally linked, replayable from any decision point under alternate policy / memory / model.

User-facing packages (see [`/packages`](../packages)):

- `@actantdb/mastra` — the wrapper for Mastra agents (the wedge entry point)
- `@actantdb/core` — embedded TS runtime (NAPI-RS native + WASM fallback — TS devs never see Rust)
- `@actantdb/types` — generated from `crates/actant-contracts` (single source of truth)
- `@actantdb/policy` — Guard verdict builders
- `@actantdb/replay` — replay engine
- `@actantdb/studio` — UI + `actantdb` CLI

Internal crates (invisible to TS developers; bundled inside `@actantdb/core`):

- `actant-contracts` — every public type lives here once
- `actant-kernel` — Rust implementation
- `actant-napi` — Node native addon
- `actant-wasm` — WASM fallback

One milestone target: **2026-06-30** — wrapper works, Studio renders timeline + replay, 3 non-Wes developers have run it on a real agent.

## The wedge in one sentence

> Actant lets you see, approve, and replay why a production agent took an action — without replacing Mastra, Convex, or your existing stack.
