# wedge/

The wedge frame stays useful: it names what an external developer should see first. The substrate underneath (built out in parallel — see [`/PIVOT.md`](../PIVOT.md)) does not change the wedge's job, which is to make the first 10 minutes obvious.

## Three runnable demos (`/wedge/demo*`)

Each is its own pnpm package with a recorded `.actantdb/<project>/events.sqlite` so the demo opens in Studio without re-running.

| Package                        | What it shows                                                            |
| ------------------------------ | ------------------------------------------------------------------------ |
| [`demo/`](./demo)              | The killer "Why did this agent delete the wrong file?" walkthrough on Mastra. |
| [`demo-langgraph/`](./demo-langgraph) | Same wedge, LangGraph-shaped router agent. Proves the wrapper isn't Mastra-specific. |
| [`demo-cli/`](./demo-cli)      | Pure-CLI variant for the keep-it-tiny crowd.                              |

Run any of them:

```bash
pnpm --filter actant-demo-test-cleanup demo
ACTANTDB_STORE_DIR=./wedge/demo/.actantdb \
  npx actantdb studio --project demo-test-cleanup
```

## Wedge docs

The operational frame and the binding constraints that hold across the substrate:

1. [`f2-f3-prevention.md`](./f2-f3-prevention.md) — **binding** product constraints (TS-native default, contract-first build). Held through the substrate buildout.
2. [`positioning.md`](./positioning.md) — one-sentence answer, HN objection answer, positioning map.
3. [`killer-demo.md`](./killer-demo.md) — the demo that defines the wedge (rehearsed in `/wedge/demo/`).
4. [`validation-tests.md`](./validation-tests.md) — cold README, install, switch tests. Pass/fail thresholds.
5. [`kill-criteria.md`](./kill-criteria.md) — the original three hard gates. Current status: see [`/GATES.md`](../GATES.md).
6. [`metrics.md`](./metrics.md) — completed external replays is the one metric. Stars don't count.
7. [`distribution-plan.md`](./distribution-plan.md) — Mastra first, Convex second, cross-framework later.
8. [`60-day-plan.md`](./60-day-plan.md) — the original day-by-day plan (the implementation half is now closed; the install-sprint half is open in [`RELEASE_CHECKLIST.md`](../RELEASE_CHECKLIST.md)).
9. [`anti-scope.md`](./anti-scope.md) — was a binding list of what NOT to build before the wedge proved out. Now superseded by reality (the substrate is built); kept as historical reference.

## The wedge in 30 seconds

Two features at the surface:

- **Guard Authority** — runtime gate on every tool call. Allow / constrain / require approval / block / halt. Policy snapshot + audit evidence.
- **Chronicle Replay** — model call + tool call + context manifest + approval + effect result, all causally linked, replayable from any decision point under alternate policy / memory / model.

What's installed when you `npm install @actantdb/mastra`:

- `@actantdb/mastra` — the wrapper.
- `@actantdb/core` — local SQLite ledger (`node:sqlite`), hash-chained events, ULIDs, idempotency.
- `@actantdb/types` — generated from `crates/actant-contracts`.
- `@actantdb/policy` — Guard verdict builders.
- `@actantdb/replay` — checkpoint / run / diff.
- `@actantdb/studio` — local UI + `actantdb` CLI.

The substrate (Rust crates, Postgres backend, Helm chart, JWT/OIDC, audit export, MCP/A2A/AP2, hot kernel, reliability primitives, observability) is in `/crates` and `/specs`. None of it is in the install path. All of it is available when an embedded run outgrows local mode.

## The wedge in one sentence

> ActantDB lets you see, approve, and replay why a production agent took an action — without replacing Mastra, Convex, or your existing stack.
