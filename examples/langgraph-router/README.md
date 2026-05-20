# actant-demo-langgraph-router

**Cross-framework demo — same wrapper, different agent shape.**

A routing agent shaped like a LangGraph node. Has `tools` and an `invoke()` instead of `generate()`. The wrapper doesn't care. This consumes [`@actantdb/langgraph`](../../packages/actant-langgraph) by package name.

## Run it

```bash
pnpm install
node demo.mjs
npx actantdb studio --project demo-langgraph-router --store-dir ./.actantdb
```

Open http://127.0.0.1:4555. You'll see:

- one `http.get` to `example.com` (allowed — low risk)
- one `shell.run` of `rm -rf cache dist` (Guard demanded approval, constrained to `rm -rf cache`)

## What this proves vs. the Mastra demo

The Mastra demo proves the wedge captures + replays one specific framework. This demo proves the LangGraph package uses the same thin `withActant` pattern for any graph or node loop that exposes `tools: Record<string, { execute }>`.

## Package boundary

`@actantdb/langgraph` is a compatibility package, not a second Guard or ledger
implementation.
