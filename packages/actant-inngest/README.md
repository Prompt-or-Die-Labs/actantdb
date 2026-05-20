# @actantdb/inngest

Named Inngest adapter for actantdb. It wraps an Inngest-shaped handler with an embedded ActantDB run ledger and does not depend on `inngest`.

```ts
import { withActantInngest } from "@actantdb/inngest";

export const handler = withActantInngest(
  async ({ event, step }) => {
    return { ok: true, id: event.data.id };
  },
  { project: "support-agent" },
);
```
