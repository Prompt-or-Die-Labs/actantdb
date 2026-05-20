# @actantdb/triggerdev

Named Trigger.dev adapter for actantdb. It wraps a task-shaped handler with an embedded ActantDB run ledger and does not depend on `@trigger.dev/sdk`.

```ts
import { withActantTriggerTask } from "@actantdb/triggerdev";

export const task = withActantTriggerTask(
  async ({ payload }) => {
    return { ok: true, id: payload.id };
  },
  { project: "support-agent" },
);
```
