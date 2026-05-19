# 09 — Add ActantDB to an existing Mastra app

You already have a Mastra agent in production. You want to add the
ledger, the Guard policy, and replay without rewriting the agent.

## TL;DR

```bash
npm install @actantdb/mastra @actantdb/policy
```

Wrap your existing agent with `withActant`:

```ts
// Before
const result = await myAgent.generate({ messages: [...] });

// After
import { withActant } from "@actantdb/mastra";
import { demoPolicy } from "@actantdb/policy";

const wrapped = withActant(myAgent, {
  project: "prod-support-agent",
  policy: demoPolicy,
});
const ctx = wrapped.startRun({ meta: { userId } });
const result = await myAgent.generate({ messages: [...] });
ctx.finish({ ok: result.ok });
```

That's it. The wrapper:

- Replaces `myAgent.tools[name].execute` with an intercepting wrapper
  that emits `tool_call_requested`, runs Guard, emits `guard_verdict`,
  pauses on `require_approval`, and writes `tool_call_completed`.
- Leaves your tool functions, message format, and result type unchanged.

## What you get for free

Every tool call now produces an audit trail:

```
agent_run_started      → run-01HX...
user_message_received  → "Clean up the test artifacts."
context_build          → 3 included, 0 blocked
model_call             → planner: "rm -rf build dist"
tool_call_requested    → shell.run "rm -rf build dist"
guard_verdict          → require_approval (constrain to "rm -rf build")
approval_required      → blocking, hash chained
approval_decision      → alice@: approve_constrained
tool_call_started      → "rm -rf build"
tool_call_completed    → exit=0
agent_run_finished     → ok=true
```

## Hooking up tools that already exist

Mastra tools follow this shape:

```ts
const myTool = {
  id: "issue_refund",
  description: "Refund an invoice",
  inputSchema: z.object({ ... }),
  execute: async ({ context }) => { ... },
};
```

`withActant` reads the tool record by `id` and wraps `execute`. No
changes to the tool itself.

## Recording the context manifest

If you build context (RAG, memory selection) by hand, record what the
model actually saw so replay can rebuild it:

```ts
import { buildContextManifest } from "@actantdb/core";

const items = await retrieveRelevantMemories(query);
const manifest = buildContextManifest(
  items.map((m) => ({
    id: m.id,
    kind: "memory",
    source: m.source,
    sensitivity: m.sensitivity ?? "low",
    label: m.label,
    content: m.text,
  })),
);
ctx.recordContextBuild(manifest);
```

## Recording model calls

Mastra's `generate()` already invokes models. To put those calls in the
ledger:

```ts
import { sha256OfJSON } from "@actantdb/core";

const prompt = { messages: [...] };
const out = await myAgent.generate(prompt);
ctx.recordModelCall({
  model: "gpt-4o-mini",
  role: "planner",
  prompt_hash: sha256OfJSON(prompt),
  summary: out.text?.slice(0, 80) ?? "(no text)",
});
```

If you'd rather not modify the agent code, hook the Mastra telemetry
exporter instead — `actant-trace` consumes OpenInference spans and
emits the same `model_call` events for you.

## Existing tests

If you had tests using vitest/jest, swap any "remember to assert these
tool calls were made" patterns for `@actantdb/testing` —
[recipe 05](./05-test-an-agent-with-snapshots.md) walks through the
migration.

## Production checklist

1. Pin `node:sqlite` (Node ≥22.5; ≥24 for unflagged).
2. Configure `ACTANTDB_STORE_DIR` so the ledger lands on a durable
   volume, not ephemeral container disk.
3. Add `actantdb studio` to your dev-only deps; expose it behind auth
   in staging if you want a hosted timeline.
4. Decide: keep `mode: "embedded"` (one ledger per process) or stand
   up an `actantdb-server` and switch to `mode: "server"` once you
   need multi-user approvals or cluster sync.

## See also

- [Recipe 01](./01-add-approval-to-a-tool.md) — add a Guard rule once the ledger is in place.
- [Recipe 03](./03-wire-into-nextjs.md) — same wrapper, but inside Next.js routes.
