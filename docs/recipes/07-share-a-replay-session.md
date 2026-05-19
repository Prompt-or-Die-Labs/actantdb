# 07 — Share a replay session

The killer demo: someone says "the agent did something weird, can you
take a look?" You want to send them a single URL (or single file) that
opens the exact run and the exact replay you were inspecting.

## Option A — share a SQLite file

The whole ledger is one SQLite file. Compress and send it.

```bash
# On your machine
PROJECT=support
DIR=~/.actantdb/$PROJECT
tar -czf /tmp/$PROJECT.tar.gz -C $DIR events.sqlite
```

The recipient extracts, opens Studio, and clicks the same run:

```bash
mkdir -p ~/.actantdb/shared-support
tar -xzf support.tar.gz -C ~/.actantdb/shared-support/
ACTANTDB_STORE_DIR=~/.actantdb npx actantdb studio --project shared-support
```

## Option B — share a JSON replay bundle

If you want to send just the replay (not the whole ledger), serialize
`ReplayRun`:

```js
// share-replay.mjs
import { writeFileSync } from "node:fs";
import { openLedger } from "@actantdb/core";
import { runFromEvent } from "@actantdb/replay";

const ledger = openLedger({ project: "support" });
const replay = runFromEvent({
  ledger,
  eventId: process.argv[2],          // the event you anchored the replay at
  overrides: { without_memory: process.argv.slice(3) },
});

writeFileSync("/tmp/replay.json", JSON.stringify(replay, null, 2));
```

The recipient renders it however they want — Studio doesn't import
free-floating `ReplayRun` JSON yet, but the schema is documented and
your in-house tooling can `JSON.parse` it directly.

## Option C — Studio URL

If Studio is running and reachable on a shared host:

```
http://studio.internal:4173/?project=support&run=<runId>
```

Studio reads `runId` from the query string and jumps straight to that
timeline. Add `&event=<eventId>` to land on the replay-anchor event.

## Option D — gist the diff

For a really lightweight share — paste it in Slack:

```js
import { openLedger } from "@actantdb/core";
import { runFromEvent, diffReplayAgainstOriginal } from "@actantdb/replay";

const ledger = openLedger({ project: "support" });
const replay = runFromEvent({ ledger, eventId: process.argv[2] });
const diff = diffReplayAgainstOriginal(ledger, replay);

for (const row of diff.entries) {
  console.log(row.diff.padEnd(10), row.kind);
}
```

Output is grep-friendly:

```
identical  agent_run_started
identical  user_message_received
changed    model_call          ← here's what changed
changed    tool_call_requested
identical  tool_call_completed
```

## A note on sensitive content

The ledger stores the model context, including any documents or memories
you sent to the model. Before sharing a ledger file, **scrub by
sensitivity**:

```js
import { openLedger } from "@actantdb/core";
const ledger = openLedger({ project: "support" });
const safe = ledger
  .query({})
  .filter((e) => e.sensitivity === "public" || e.sensitivity === "low");
// write `safe` to a new file or post it inline; everything `medium+` stays put.
```

For automated redaction at write time, configure the `sensitivity_ceiling`
on your policy so high-sensitivity content never reaches the ledger.

## See also

- [Recipe 02](./02-replay-last-nights-failed-run.md) — how the replay you're sharing was generated.
- [Recipe 03](./03-wire-into-nextjs.md) — host a per-user Studio inside your app.
