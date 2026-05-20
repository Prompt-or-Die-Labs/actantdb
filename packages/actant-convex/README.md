# @actantdb/convex

Adapter for Convex-shaped handlers. The shape mirrors `@actantdb/mastra`:

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

See [`/PIVOT.md`](../../PIVOT.md) and [`/CHANGELOG.md`](../../CHANGELOG.md).
