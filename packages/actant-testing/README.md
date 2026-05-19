# @actantdb/testing

Testing helpers for consumers of ActantDB. Drop into any test runner
(Vitest, Jest, Node's built-in `node:test`).

## Install

```bash
npm install --save-dev @actantdb/testing
```

## Quick start

```ts
import { describe, it } from "vitest";
import {
  createTestLedger,
  expectEventEmitted,
  expectGuardVerdict,
} from "@actantdb/testing";

describe("my agent", () => {
  it("requires approval for big refunds", async () => {
    const t = createTestLedger();
    try {
      await runMyAgent({ ledger: t.ledger, message: "refund $42 to INV-001" });

      expectEventEmitted(t, "tool_call_completed", {
        tool_call_id: "tc1",
        status: "ok",
      });
      expectGuardVerdict(t, {
        tool_name: "issue_refund",
        decision: "require_approval",
      });
    } finally {
      t.close();
    }
  });
});
```

## API

### `createTestLedger(options?)`

Constructs an in-memory `Ledger` (`:memory:` SQLite, never touches disk).
Returns a `TestLedger` with helpers for appending the common event kinds:

```ts
const t = createTestLedger({ project: "p" });
const runId = t.newRun();
t.appendUserMessage(runId, "hello");
t.appendToolCallRequested(runId, {
  tool_call_id: "tc1",
  tool: "issue_refund",
  risk: "high",
  args: { amount_cents: 4200 },
});
t.appendGuardVerdict(runId, "tc1", {
  decision: "require_approval",
  policy_snapshot: "p-hash",
  reason: "refund > $20 requires approval",
});
```

The `.ledger` is a plain `@actantdb/core` `Ledger`, so any consumer that
takes a ledger handle accepts it as-is.

### `expectEventEmitted(source, kind, payloadMatch?)`

Throws `AssertionError` if no event with `kind` matching the partial
`payloadMatch` is in the ledger. `source` may be a `TestLedger`, a bare
`Ledger`, or an `ActantEvent[]`.

### `expectEventNotEmitted(source, kind, payloadMatch?)`

Inverse of the above.

### `expectGuardVerdict(source, { tool_name?, decision?, reason_includes? })`

Asserts a `guard_verdict` was emitted matching the criteria. Pairs the
verdict to the preceding `tool_call_requested` so `tool_name` works
intuitively.

### `expectToolCompleted(source, { tool_name?, status? })`

Asserts a `tool_call_completed` matching the criteria. Pairs to the
upstream tool request to make `tool_name` meaningful.

### `expectChainIntact(source)`

Walks every event and verifies `payload_hash` + `chain_hash` are well
formed.

### `findEvents(source, kind, payloadMatch?)`

Returns every match (use this when you want to inspect more than just
the first).

### Snapshots

```ts
import { snapshotEvents } from "@actantdb/testing";

expect(snapshotEvents(t.events())).toMatchInlineSnapshot();
```

`snapshotEvents` strips churn-y fields (`duration_ms`, `timestamp`,
`trace_id`, `span_id`) by default. Pass `scrubKeys` to add more.

## Conventions

- Helpers throw `AssertionError` (the named export) — test runners will
  surface the message verbatim.
- Helpers accept either a `TestLedger`, a `Ledger`, or `ActantEvent[]`,
  so they compose with custom helpers in your project.
