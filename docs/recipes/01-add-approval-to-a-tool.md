# 01 — Add an approval gate to a tool

You have a tool with side effects (refund money, send email, run a shell
command). You want a human to confirm before it fires for high-risk inputs.
ActantDB's policy DSL turns that into one declarative rule and a single
ledger event you can act on.

## The mechanism

1. Define a `Policy` with a per-tool risk (`high` / `destructive`) or a
   `deny`-rule pattern.
2. Wrap your agent with `withActant({ policy })`. Guard evaluates the
   policy against every `tool_call_requested` event.
3. When the verdict is `require_approval`, Guard appends an
   `approval_required` event and pauses the call. Your UI (or another
   agent) calls `recordApprovalDecision` with `decision: "approve"` or
   `"deny"`.

## Minimal example

```js
// approval.mjs
import { withActant } from "@actantdb/mastra";
import { verdict } from "@actantdb/policy";

const policy = {
  tools: [
    { tool: "issue_refund", risk: "high", require_approval: true },
    { tool: "delete_user",  risk: "destructive", require_approval: true },
  ],
  deny: [
    {
      tool: "shell.run",
      pattern: "rm -rf /",
      reason: "no full-disk wipes, ever",
    },
  ],
};

const agent = {
  name: "support-agent",
  tools: {
    issue_refund: {
      id: "issue_refund",
      description: "Refund an invoice",
      execute: async ({ invoice, amount_cents }) => {
        // ... call Stripe ...
        return { refunded_cents: amount_cents, invoice };
      },
    },
  },
  generate: async () => "noop",
};

const wrapped = withActant(agent, { project: "support", policy });
```

## Handling the approval

```js
import { demoPolicy } from "@actantdb/policy";

const ledger = wrapped.actant.ledger;
const approvals = wrapped.actant.approvals;

// Anywhere — another process, a Studio click, an agent peer — find pending:
for (const req of approvals.pending()) {
  console.log("pending:", req.toolCallId, req.request.reason);
  approvals.decide(req.toolCallId, {
    decision: "approve",
    approver: "alice@example.com",
    scope: "single-call",
  });
}
```

The decision lands in the ledger as an `approval_decision` event and Guard
releases the tool call.

## `approve_constrained` — accept a narrower input

Sometimes the right answer is "yes, but only for $20, not $200". Guard
emits `decision: "constrain"` if the policy supplies a `constrained_input`
hint, and `withActant({ autoApprove: true })` accepts the constrained
variant automatically (useful for tests). To do this by hand:

```js
approvals.decide(req.toolCallId, {
  decision: "approve_constrained",
  approver: "alice@example.com",
  scope: "single-call",
  accepted_input: { invoice: "INV-001", amount_cents: 2000 },
});
```

## What you get on the audit side

Replay the run with a stricter policy (`@actantdb/replay` `tighten`)
to prove the approval was the right call:

```js
import { runFromEvent, tighten } from "@actantdb/replay";

const stricter = tighten(policy, {
  deny: [{ tool: "issue_refund", pattern: ".*", reason: "no refunds, period" }],
});

const replay = runFromEvent({
  ledger,
  eventId: someToolRequestEventId,
  policy: stricter,
});
// replay.events shows what would have happened without the approval path.
```

## See also

- [Recipe 05](./05-test-an-agent-with-snapshots.md) — assert the verdict in a unit test.
- [Recipe 07](./07-share-a-replay-session.md) — share the resulting timeline.
