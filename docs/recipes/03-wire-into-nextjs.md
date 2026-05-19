# 03 — Wire ActantDB into Next.js

Embed the ledger inside a Next.js app so every API-route agent run is
auditable, replayable, and viewable in Studio without standing up a
separate service.

## Layout

```
app/
├── api/
│   ├── chat/route.ts          ← your agent route
│   └── actantdb/
│       ├── events/route.ts     ← read-only events endpoint
│       └── studio/[...path]/route.ts  ← optional Studio proxy
└── lib/actantdb.ts             ← shared handle (1 per process)
```

The trick is to construct **one** `Actant` per process — Next.js spawns
a worker per route in dev, so module-singleton pattern is the cleanest
approach.

## Shared handle

```ts
// app/lib/actantdb.ts
import { createActant } from "@actantdb/core";
import { demoPolicy } from "@actantdb/policy";

// Module-level singleton survives HMR by stashing it on globalThis.
const g = globalThis as { __actant?: ReturnType<typeof createActant> };

export const actant =
  g.__actant ??
  (g.__actant = createActant({
    project: "next-app",
    storeDir: process.env.ACTANTDB_STORE_DIR ?? "./.actantdb",
    policy: demoPolicy,
  }));
```

`@actantdb/core` uses `node:sqlite`, which is unflagged on Node ≥24.
Make sure your Next.js runtime is `nodejs` (not `edge`) by exporting:

```ts
export const runtime = "nodejs";
```

at the top of every route that imports `@actantdb/core`.

## Agent route

```ts
// app/api/chat/route.ts
import { actant } from "@/app/lib/actantdb";
import { NextRequest } from "next/server";

export const runtime = "nodejs";

export async function POST(req: NextRequest) {
  const { message } = await req.json();

  const ctx = actant.startRun({ meta: { source: "next-chat" } });
  ctx.recordUserMessage(message);
  // ... drive your agent (Mastra, LangGraph, ad hoc) and record events ...
  ctx.recordModelCall({
    model: "gpt-4o-mini",
    role: "planner",
    prompt_hash: "h",
    summary: "responded with greeting",
  });
  ctx.finish({ ok: true });

  return Response.json({ runId: ctx.runId });
}
```

## Read-only events endpoint

Expose the ledger to your frontend so the UI can render a timeline.

```ts
// app/api/actantdb/events/route.ts
import { actant } from "@/app/lib/actantdb";
import { NextRequest } from "next/server";

export const runtime = "nodejs";

export async function GET(req: NextRequest) {
  const sp = req.nextUrl.searchParams;
  const runId = sp.get("run");
  if (!runId) return Response.json({ error: "missing run" }, { status: 400 });
  const events = actant.ledger.query({ runId, limit: 500 });
  return Response.json({ events });
}
```

## Studio in dev only

In dev, run Studio separately (`npx actantdb studio --project next-app
--store-dir ./.actantdb`) — the browser at `http://localhost:4173` is the
intended UI surface. In production, you almost never want Studio open to
the public; gate it behind auth or skip it entirely.

If you want Studio inline, mount it under a proxied catch-all route:

```ts
// app/api/actantdb/studio/[...path]/route.ts
import { actant } from "@/app/lib/actantdb";
import { startStudioServer } from "@actantdb/studio";

export const runtime = "nodejs";

const studio = await startStudioServer({ ledger: actant.ledger, port: 0, silent: true });

export async function GET(req: Request) {
  const url = new URL(req.url);
  const fwd = new URL(url.pathname.replace("/api/actantdb/studio", "") + url.search, studio.url);
  const r = await fetch(fwd);
  return new Response(r.body, { status: r.status, headers: r.headers });
}
```

## Vercel deployment notes

- `@actantdb/core` uses `node:sqlite`. Vercel's Node runtime supports it
  on Node 24+; pin the project runtime explicitly in `package.json`'s
  `"engines"`.
- The SQLite file is **not** durable across deploys on serverless. Mount
  a real filesystem (Fly Volumes, a Postgres backend via
  `actant-storage`, or pair-mode: write to a remote `actantdb-server`).

## See also

- [Recipe 09](./09-add-actantdb-to-an-existing-mastra-app.md) — if your Next.js app already uses Mastra.
- [Recipe 10](./10-build-your-first-mcp-tool-on-top-of-actantdb.md) — expose the same ledger to Claude Desktop.
