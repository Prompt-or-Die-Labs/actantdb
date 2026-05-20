# @actantdb/replay

Replay engine: checkpoint, run-from-event, diff, override (policy / memory / model).

v0.1 scope (per [`/CHANGELOG.md`](../../CHANGELOG.md) days 36–50):

```ts
import { Replay } from "@actantdb/replay";

const r = new Replay({ project: "support-agent" });
const checkpoint = await r.checkpoint(eventId);
const run = await r.run(checkpoint, {
  withoutMemory: ["mem_42"],
  policy: "strict",
});
const diff = await r.diff(originalRunId, run.id);
```

Replay does NOT re-execute real side effects in v0.1. Tool results in replay mode are reused from the recorded `effect_result`. Experimental re-invocation stays behind an explicit mode.

All public types come from [`@actantdb/types`](../actant-types) (generated from `crates/actant-contracts`).
