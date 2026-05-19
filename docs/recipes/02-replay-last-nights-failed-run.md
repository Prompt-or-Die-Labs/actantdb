# 02 — Replay last night's failed run

A scheduled agent failed at 3am. The trace tells you which model call
errored. ActantDB tells you why the model *chose* to call it — and lets
you replay the same decision with one variable changed.

## Sequence

1. Find the run id of the failure (Studio, `ledger.query`, or your tracing UI).
2. Pick a checkpoint event (usually the `model_call` right before the failure).
3. Run `runFromEvent(ledger, eventId, overrides)` with the override you
   want to test (drop a memory, use a different model, tighten the policy).
4. Diff against the original with `diffReplayAgainstOriginal`.

## Find the run

```js
// find-failed-run.mjs
import { openLedger } from "@actantdb/core";

const ledger = openLedger({ project: "nightly-cleanup" });
const failures = ledger.query({ kind: "tool_call_completed" }).filter(
  (e) => e.payload.status === "error",
);
console.log(`found ${failures.length} failed tool calls`);
for (const f of failures.slice(-5)) {
  console.log(f.created_at, f.run_id, f.payload.tool_call_id);
}
```

## Replay with a memory excluded

```js
// replay.mjs
import { openLedger } from "@actantdb/core";
import { runFromEvent, diffReplayAgainstOriginal, tighten } from "@actantdb/replay";
import { demoPolicy } from "@actantdb/policy";

const ledger = openLedger({ project: "nightly-cleanup" });
const runId = process.argv[2];        // from the previous script
const memoryToBlame = process.argv[3] ?? "mem_42_dist";

// Pick the model_call event immediately before the bad tool call.
const events = ledger.query({ runId });
const modelCall = events.find((e) => e.kind === "model_call");
if (!modelCall) throw new Error("no model_call in this run");

const replay = runFromEvent({
  ledger,
  eventId: modelCall.id,
  overrides: { without_memory: [memoryToBlame] },
  policy: tighten(demoPolicy, {
    deny: [
      { tool: "shell.run", pattern: "rm -rf", reason: "no full deletes in replay" },
    ],
  }),
});

const diff = diffReplayAgainstOriginal(ledger, replay);
for (const row of diff.entries) {
  console.log(row.diff.padEnd(10), row.kind);
}
```

## Replay with a substituted tool result

Sometimes you want to ask "what if the API had returned 200 instead of
500?". Use `mode: "tool"`:

```js
const replay = runFromEvent({
  ledger,
  eventId: modelCall.id,
  mode: "tool",
  toolSubstitutions: {
    "tc-the-failing-id": { ok: true, data: { id: "fake-id" } },
  },
});
```

The substituted completion is marked `status: "substituted"` in the
replay's event stream, so downstream consumers can distinguish replayed
from recorded outcomes.

## Replay with a re-invocation (experimental)

If the tool is read-only and safe to actually invoke again under controlled
inputs, use `mode: "experimental"`:

```js
const replay = runFromEvent({
  ledger,
  eventId: modelCall.id,
  mode: "experimental",
  experimentalToolCallId: "tc-the-failing-id",
  experimentalReplacementResult: { ok: true },
});
```

`experimentalReplacementResult` is what the tool would have returned;
downstream events are marked so the diff surfaces them as `changed`.

## Open the diff in Studio

```bash
npx actantdb studio --project nightly-cleanup
```

Click the run, click the model call, click **Replay**. Studio reads the
same `runFromEvent` API and renders the diff side-by-side.

## See also

- [Recipe 01](./01-add-approval-to-a-tool.md) — add a Guard rule that would have caught it next time.
- [Recipe 07](./07-share-a-replay-session.md) — share the replay with a teammate.
