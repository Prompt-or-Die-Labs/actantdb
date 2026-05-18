# @actantdb/convex

**Conditional, not Phase 1.** Lives here as a placeholder; it ships only if a design partner is on Convex.

When it ships, the shape is the same as `@actantdb/mastra`:

```ts
import { withActant } from "@actantdb/convex";

export const agent = withActant(myConvexAgent, {
  project: "support-agent",
  replay: true,
  approvals: true,
});
```

Convex's own durable workflows + reactive agent state stay in place; `@actantdb/convex` adds:

- the chronicle ledger
- runtime authority gate (constrain / approve / deny)
- replay from any workflow step
- context manifest for every model call inside the workflow

Until the Mastra wedge has passed Gate 1 (`/wedge/kill-criteria.md`) and at least one design partner uses Convex, this package is not actively built.

See [`/PIVOT.md`](../../PIVOT.md) and [`/wedge/distribution-plan.md`](../../wedge/distribution-plan.md).
