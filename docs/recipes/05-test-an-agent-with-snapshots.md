# 05 — Test an agent with snapshots

Agents are stateful, side-effecting, and call multiple models. Unit-testing
them with `expect(toolCalls).toEqual([...])` quickly degenerates into
fragile asserting on bag-of-args. ActantDB gives you something better: the
ledger is the canonical event stream; you snapshot it.

## Setup

```bash
npm install --save-dev @actantdb/testing vitest
```

## A first test

```ts
// agent.test.ts
import { describe, expect, it } from "vitest";
import {
  createTestLedger,
  expectEventEmitted,
  expectGuardVerdict,
  snapshotEvents,
} from "@actantdb/testing";

import { runMyAgent } from "../src/agent.ts";

describe("refund agent", () => {
  it("emits the expected event sequence", async () => {
    const t = createTestLedger({ project: "refund-test" });
    try {
      await runMyAgent({
        ledger: t.ledger,
        message: "refund invoice INV-001 for $42",
      });

      expectEventEmitted(t, "tool_call_completed", {
        tool_call_id: "tc-refund-1",
        status: "ok",
      });
      expectGuardVerdict(t, {
        tool_name: "issue_refund",
        decision: "require_approval",
      });

      // Snapshot the whole event stream, with churn-y fields scrubbed.
      expect(snapshotEvents(t.events())).toMatchInlineSnapshot();
    } finally {
      t.close();
    }
  });
});
```

## What `createTestLedger` gives you

- A `Ledger` backed by `:memory:` SQLite. No disk I/O, no `~/.actantdb`
  pollution, no cleanup between tests.
- Helpers (`appendUserMessage`, `appendToolCallRequested`,
  `appendApprovalDecision`, ...) so you can drive the ledger without
  reaching into the `Ledger` API.

## What `expect*` gives you

`expectEventEmitted(source, kind, payloadMatch?)` is the workhorse — it
asserts at least one event with that `kind` exists, where the optional
`payloadMatch` is a partial deep-match against `payload`.

The specialized helpers (`expectGuardVerdict`, `expectToolCompleted`)
walk the ledger to pair a `guard_verdict` to its `tool_call_requested`,
so you can say "the verdict on `issue_refund` was `require_approval`"
without knowing the auto-generated `tool_call_id`.

## Snapshot stability

`snapshotEvents` strips the churn-y fields by default
(`duration_ms`, `timestamp`, `trace_id`, `span_id`). That alone is
usually enough for stable snapshots. If your payloads include other
fields that change run-to-run (`request_id`, `nonce`), pass them in:

```ts
expect(
  snapshotEvents(t.events(), { scrubKeys: ["request_id", "nonce"] })
).toMatchInlineSnapshot();
```

The snapshot output preserves `kind`, `run_id`, `parent_event_id`,
`sensitivity`, and the (pruned) payload — enough to verify the agent's
shape, not its timing.

## A property test: chain integrity

```ts
import { expectChainIntact } from "@actantdb/testing";

it("never breaks the hash chain", async () => {
  const t = createTestLedger();
  try {
    for (let i = 0; i < 50; i++) {
      await runMyAgent({ ledger: t.ledger, message: `iter ${i}` });
    }
    expectChainIntact(t);
  } finally {
    t.close();
  }
});
```

`expectChainIntact` verifies `payload_hash` and `chain_hash` shapes on
every event — useful as a smoke test for changes to your ledger writer.

## See also

- [Recipe 02](./02-replay-last-nights-failed-run.md) — replay a failure in the same harness.
- [Recipe 10](./10-build-your-first-mcp-tool-on-top-of-actantdb.md) — let Claude run your tests via MCP.
