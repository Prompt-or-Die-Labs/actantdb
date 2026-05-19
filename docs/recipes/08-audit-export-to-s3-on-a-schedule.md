# 08 — Audit-export to S3 on a schedule

Push a daily snapshot of the ledger to S3 so compliance, security, and
data-retention requirements have an immutable trail. Pairs with the
hash-chained `chain_hash` for tamper evidence — if S3 and the live
ledger disagree, you know.

## What ships

- `crates/actant-audit-export` — Rust crate; the canonical scheduled
  exporter for the Rust server. It writes per-workspace ndjson dumps
  with retention enforcement.
- `crates/actant-sync` — pluggable destinations (filesystem, S3, GCS,
  Azure, IPFS) with cursor-based incremental sync.

This recipe is the **TypeScript local-first** path: a small Node script
that dumps to ndjson and pushes to S3, driven by cron.

## Script

```js
// audit-export.mjs
import { createReadStream, createWriteStream, statSync } from "node:fs";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";

import { S3Client, PutObjectCommand } from "@aws-sdk/client-s3";

import { openLedger } from "@actantdb/core";

const PROJECT = process.env.ACTANTDB_PROJECT ?? "support";
const BUCKET = process.env.AUDIT_BUCKET ?? "acme-agent-audit";
const PREFIX = process.env.AUDIT_PREFIX ?? "actantdb";

const ledger = openLedger({ project: PROJECT });
const date = new Date().toISOString().slice(0, 10);
const stagingDir = mkdtempSync(join(tmpdir(), "actantdb-audit-"));
const outPath = join(stagingDir, `${PROJECT}-${date}.ndjson`);
const out = createWriteStream(outPath);

let cursor = "";
let count = 0;
for (;;) {
  const events = ledger.query({ sinceId: cursor, limit: 1000 });
  if (events.length === 0) break;
  for (const e of events) {
    out.write(JSON.stringify(e) + "\n");
    cursor = e.id;
    count += 1;
  }
}
await new Promise((resolve) => out.end(resolve));

const size = statSync(outPath).size;
const sha = await sha256OfFile(outPath);
console.log(`audit-export: ${count} events, ${size} bytes, sha256=${sha}`);

const s3 = new S3Client({});
const key = `${PREFIX}/${PROJECT}/${date}/events.ndjson`;
await s3.send(
  new PutObjectCommand({
    Bucket: BUCKET,
    Key: key,
    Body: createReadStream(outPath),
    Metadata: {
      "actantdb-event-count": String(count),
      "actantdb-sha256": sha,
      "actantdb-project": PROJECT,
    },
    ServerSideEncryption: "AES256",
    // Object Lock for compliance, if your bucket has it configured.
    // ObjectLockMode: "COMPLIANCE",
    // ObjectLockRetainUntilDate: new Date(Date.now() + 7 * 365 * 86400000),
  }),
);
console.log(`audit-export: s3://${BUCKET}/${key}`);

async function sha256OfFile(path) {
  return await new Promise((resolve, reject) => {
    const h = createHash("sha256");
    createReadStream(path)
      .on("error", reject)
      .on("data", (c) => h.update(c))
      .on("end", () => resolve(h.digest("hex")));
  });
}
```

## Cron

```bash
# crontab
0 2 * * * /usr/bin/env node /opt/actantdb/audit-export.mjs >> /var/log/actantdb-audit.log 2>&1
```

For systemd timers:

```ini
# /etc/systemd/system/actantdb-audit.service
[Unit]
Description=ActantDB audit export to S3

[Service]
Type=oneshot
EnvironmentFile=/etc/actantdb/audit.env
ExecStart=/usr/bin/env node /opt/actantdb/audit-export.mjs
```

```ini
# /etc/systemd/system/actantdb-audit.timer
[Unit]
Description=Run ActantDB audit export nightly

[Timer]
OnCalendar=daily
RandomizedDelaySec=30min
Persistent=true

[Install]
WantedBy=timers.target
```

## Verify tamper evidence

Daily dumps include the `chain_hash` of every event. To verify a backup
matches the live ledger:

```js
import { createReadStream } from "node:fs";
import { createInterface } from "node:readline";
import { openLedger } from "@actantdb/core";

const ledger = openLedger({ project: process.env.ACTANTDB_PROJECT });
const liveByid = new Map(ledger.query({}).map((e) => [e.id, e.chain_hash]));

const rl = createInterface({ input: createReadStream("/tmp/dump.ndjson") });
let mismatches = 0;
for await (const line of rl) {
  const e = JSON.parse(line);
  if (liveByid.get(e.id) !== e.chain_hash) {
    mismatches += 1;
    console.warn("MISMATCH", e.id, e.kind);
  }
}
console.log(`${mismatches} mismatches`);
```

Zero mismatches = the backup is consistent with the live chain. Any
nonzero is an integrity event worth investigating.

## Retention

Pair with S3 lifecycle rules to age out exports older than your
retention window:

```json
{
  "Rules": [
    {
      "ID": "actantdb-audit-7y",
      "Status": "Enabled",
      "Filter": { "Prefix": "actantdb/" },
      "Expiration": { "Days": 2555 }
    }
  ]
}
```

## See also

- [Recipe 06](./06-export-to-bigquery.md) — same data, different sink.
- [Recipe 04](./04-use-ollama-only-no-cloud-models.md) — pair with a sensitivity ceiling so audit dumps stay PII-free.
