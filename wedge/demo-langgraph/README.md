# actant-demo-langgraph-router

**Cross-framework demo — same wrapper, different agent shape.**

A routing agent shaped like a LangGraph node. Has `tools` and an `invoke()` instead of `generate()`. The wrapper doesn't care. This proves [`@actantdb/mastra`](../../packages/actant-mastra)'s duck-typing claim.

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

The Mastra demo proves the wedge captures + replays one specific framework. This demo proves the **wrapper is framework-agnostic**: it works on any agent that exposes `tools: Record<string, { execute }>`. That's the F2-prevention property — Mastra users today, LangGraph / OpenAI Agents SDK / hand-rolled tomorrow.

## Anti-scope

This is NOT a `@actantdb/langgraph` package. The wrapper *is* `@actantdb/mastra` — we'd ship that as a separate package only after a LangGraph design partner requests it (anti-scope rule #5).
