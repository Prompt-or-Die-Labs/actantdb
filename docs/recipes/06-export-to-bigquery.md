# 06 — Export to BigQuery

Get the ledger into a warehouse so analysts can query for "how many
refunds did the agent issue last week", "which tools required approval
most often", "what's our average run length".

## What ships in the box

`crates/actant-audit-export` is the canonical exporter — it can write
ndjson dumps suitable for `bq load`. From TypeScript, the simplest path
is to read the ledger and stream a newline-delimited JSON file, then
`bq load` it.

## Newline-delimited JSON dump

```js
// export.mjs
import { openLedger } from "@actantdb/core";
import { createWriteStream } from "node:fs";

const ledger = openLedger({ project: "support" });
const out = createWriteStream("/tmp/actantdb.ndjson");

let exported = 0;
let cursor = "";
const PAGE = 1000;

for (;;) {
  const events = ledger.query({
    sinceId: cursor,
    limit: PAGE,
  });
  if (events.length === 0) break;
  for (const e of events) {
    // Flatten payload to a top-level column for BigQuery's column-friendly
    // STRUCT inference. Keep the original ledger fields too.
    out.write(
      JSON.stringify({
        ...e,
        payload_text: JSON.stringify(e.payload),
      }) + "\n",
    );
    exported += 1;
    cursor = e.id;
  }
}

out.end();
console.log(`exported ${exported} events`);
```

## BigQuery schema

```json
[
  { "name": "id",              "type": "STRING",    "mode": "REQUIRED" },
  { "name": "kind",            "type": "STRING",    "mode": "REQUIRED" },
  { "name": "project",         "type": "STRING",    "mode": "REQUIRED" },
  { "name": "run_id",          "type": "STRING",    "mode": "REQUIRED" },
  { "name": "parent_event_id", "type": "STRING",    "mode": "NULLABLE" },
  { "name": "payload_text",    "type": "STRING",    "mode": "REQUIRED" },
  { "name": "payload_hash",    "type": "STRING",    "mode": "REQUIRED" },
  { "name": "chain_hash",      "type": "STRING",    "mode": "REQUIRED" },
  { "name": "sensitivity",     "type": "STRING",    "mode": "REQUIRED" },
  { "name": "created_at",      "type": "TIMESTAMP", "mode": "REQUIRED" }
]
```

## Load

```bash
bq load \
  --source_format=NEWLINE_DELIMITED_JSON \
  --schema=/path/to/schema.json \
  acme:agents.events_v1 \
  /tmp/actantdb.ndjson
```

For incremental loads, persist the cursor (the last `id` you exported)
to disk between runs and start the next export from there:

```js
import { readFileSync, writeFileSync, existsSync } from "node:fs";

const CURSOR = "/var/lib/actantdb-export/cursor.txt";
let cursor = existsSync(CURSOR) ? readFileSync(CURSOR, "utf8").trim() : "";

// ... export loop, updates `cursor` ...

writeFileSync(CURSOR, cursor);
```

## Sample queries

```sql
-- Tool calls per day per tool.
SELECT
  DATE(created_at) AS day,
  JSON_VALUE(payload_text, '$.tool') AS tool,
  COUNT(*) AS calls
FROM acme.agents.events_v1
WHERE kind = 'tool_call_requested'
GROUP BY day, tool
ORDER BY day DESC, calls DESC;

-- Approval rate.
SELECT
  JSON_VALUE(payload_text, '$.tool') AS tool,
  COUNT(*) AS total,
  COUNTIF(JSON_VALUE(payload_text, '$.decision') = 'approve') AS approved,
  COUNTIF(JSON_VALUE(payload_text, '$.decision') = 'deny')    AS denied
FROM acme.agents.events_v1
WHERE kind = 'approval_decision'
GROUP BY tool;

-- p95 tool latency by tool.
SELECT
  JSON_VALUE(payload_text, '$.tool_call_id') AS tcid,
  APPROX_QUANTILES(
    CAST(JSON_VALUE(payload_text, '$.duration_ms') AS INT64), 100
  )[OFFSET(95)] AS p95_ms
FROM acme.agents.events_v1
WHERE kind = 'tool_call_completed'
GROUP BY tcid;
```

## Schedule it

```bash
# crontab -e
15 4 * * * /usr/bin/env node /opt/actantdb-export/export.mjs >> /var/log/actantdb-export.log 2>&1
```

For a cluster setup with the Rust server, point `actant-audit-export`
at the running server and let it do the export; the TS path above is for
local-first deployments.

## See also

- [Recipe 08](./08-audit-export-to-s3-on-a-schedule.md) — push the same file to S3 instead.
