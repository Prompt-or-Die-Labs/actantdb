# @actantdb/workflow

Drop-in port of [Upstash Workflow](https://github.com/upstash/workflow-js) on
top of the [ActantDB](https://github.com/Prompt-or-Die-Labs/actantdb) ledger.

Same `serve()`. Same `Client`. Same `ctx.run / sleep / sleepUntil / call /
waitForEvent / notify / cancel`. Persistence is the local ActantDB ledger,
so there is no QStash, no Redis, no external service to provision. Every
step lands as an `ActantEvent` you can query and replay through
`@actantdb/replay`.

## Install

```sh
npm install @actantdb/workflow @actantdb/core
```

Runtime dependencies: `@actantdb/core` + `@actantdb/types`. Nothing else.

## Quick start (Next.js App Router)

```ts
// app/api/workflow/route.ts
import { serve } from "@actantdb/workflow";

export const POST = serve(async (ctx) => {
  const order = await ctx.run("fetch-order", () => fetchOrder(ctx.payload.id));

  await ctx.sleep("payment-grace", "5m");

  const charged = await ctx.run("charge", () =>
    stripe.charges.create({ amount: order.total, customer: order.customer }),
  );

  const shipment = await ctx.waitForEvent(
    "await-shipment",
    `shipped:${order.id}`,
    { timeout: "7d" },
  );

  await ctx.call("notify", {
    url: "https://hooks.slack.com/services/...",
    method: "POST",
    body: { text: `Order ${order.id} charged + shipped: ${shipment.tracking}` },
  });

  return { ok: true };
});
```

## Trigger a run

```ts
import { Client } from "@actantdb/workflow";

const client = new Client({ baseUrl: "https://my-app.com/api/workflow" });
const { workflowRunId } = await client.trigger({
  body: { id: "ord_42" },
});
```

## Notify a waiting run

```ts
await client.notify({
  eventId: `shipped:${orderId}`,
  eventData: { tracking: "1Z..." },
  workflowRunId, // optional — scopes the notify to one run
});
```

## Cancel a run

```ts
await client.cancel({ workflowRunId });
```

## Local mode — write directly to the ledger

Skip HTTP entirely and use a shared ledger:

```ts
import { openLedger } from "@actantdb/core";
import { Client, serve } from "@actantdb/workflow";

const ledger = openLedger({ project: "orders" });

const handler = serve(myWorkflow, { ledger });
const client = new Client({ ledger });

// Notify / cancel land directly in the ledger; the next invocation of
// `handler` for that runId picks them up.
await client.notify({ eventId: "shipped:42", eventData: { ... } });
```

## API reference

### `serve(handler, opts?)`

Returns a Web-Fetch-API handler — `(req: Request) => Promise<Response>` —
plus a programmatic `invoke({ runId?, body?, headers? })` and the bound
`ledger`. Works in Next.js, Hono, Bun, Deno, Cloudflare Workers, and any
runtime that speaks `fetch`.

| Option       | Default        | Description                                                         |
| ------------ | -------------- | ------------------------------------------------------------------- |
| `ledger`     | open new       | Reuse a `Ledger` you already opened.                                |
| `project`    | `"workflow"`   | Project id used when opening a fresh ledger.                        |
| `storeDir`   | `~/.actantdb`  | Override the default store directory.                               |
| `inMemory`   | `false`        | Open the ledger in-memory (tests).                                  |
| `autoResume` | `true`         | When a sleep suspends, schedule a `setTimeout` to retrigger.        |
| `fetch`      | `globalThis`   | Override the `fetch` used inside `ctx.call` (tests).                |

### `new Client(opts?)`

| Option    | Default    | Description                                                       |
| --------- | ---------- | ----------------------------------------------------------------- |
| `baseUrl` | —          | URL of the `serve()` handler (HTTP mode).                         |
| `ledger`  | —          | Shared `Ledger` for local mode. Wins over `baseUrl` per call.     |
| `token`   | —          | Bearer token (Upstash compat; sent on every request).             |
| `fetch`   | `globalThis` | Override the `fetch` impl.                                      |

Methods:

- `trigger({ url?, body?, headers?, workflowRunId?, retries? })` — start
  a run. Returns `{ workflowRunId }`.
- `cancel({ workflowRunId })` — abort a run.
- `notify({ eventId, eventData?, workflowRunId? })` — publish an event
  for any matching `waitForEvent`.

### Context (`ctx`)

| Method                                       | Description                                                                              |
| -------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `ctx.run(name, fn)`                          | Durable step. Result cached in the ledger; skipped on resume.                            |
| `ctx.sleep(name, "5m")`                      | Durable sleep. Suspends until the deadline passes.                                       |
| `ctx.sleepUntil(name, isoOrUnixMs)`          | Durable absolute-time sleep.                                                             |
| `ctx.call(name, { url, method, body })`      | Durable HTTP request. Response cached.                                                   |
| `ctx.waitForEvent(name, eventId, { timeout })` | Suspend until a `notify` lands. Returns the notify's data or `undefined` on timeout.   |
| `ctx.notify(eventId, data)`                  | Publish an event from inside a workflow.                                                 |
| `ctx.cancel()`                               | Abort the current run (throws an internal sentinel; the runner finalizes the ledger).    |

Read-only fields:

- `ctx.runId` — the run id.
- `ctx.payload` — the body posted to `trigger`.
- `ctx.requestHeaders` — incoming headers (HTTP mode only).

### Durations

Accepts ms (`number`) or string with unit (`100ms`, `5s`, `5m`, `3h`, `7d`).

## Upstash compatibility

This package targets a 1:1 surface match with `@upstash/workflow`'s
JS SDK. Migration:

```ts
- import { serve } from "@upstash/workflow/nextjs";
- import { Client } from "@upstash/workflow";
+ import { serve, Client } from "@actantdb/workflow";

- const client = new Client({ token: process.env.QSTASH_TOKEN });
+ const client = new Client({ baseUrl: "https://my-app.com/api/workflow" });
```

Everything inside the handler body — `ctx.run`, `ctx.sleep`, etc — stays
identical.

### Documented divergences

| Area                       | Upstash                                                       | `@actantdb/workflow`                                                                              |
| -------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Persistence backend        | QStash (Redis-backed, hosted).                                | Local ActantDB ledger (`@actantdb/core`, SQLite).                                                 |
| `ctx.call` connection model | Non-blocking — QStash holds the request and resumes you when the callee responds. | Blocking — local mode does the `fetch` synchronously and caches the response. Behavior identical for the caller, but worker CPU is tied up during the call. |
| `Client.notify`            | Returns event delivery receipts.                              | Fire-and-forget against the ledger; the next `waitForEvent` reader picks it up.                   |
| Retries                    | Configurable per call via QStash.                             | A retry is just a re-invocation with the same `runId`; the runner skips completed steps.          |
| `token` option             | Required (QStash auth).                                       | Optional — sent as `Authorization: Bearer …` if present; ignored in local mode.                   |

If you depend on any of these for production at scale, the local mode is
not a complete replacement for QStash — but for development, testing,
self-hosted single-node deploys, and any case where you want the full
event-stream introspection ActantDB gives you, it's a real drop-in.

## Replay + audit

Every step lands in the ledger as a typed `ActantEvent`:

- `ctx.run(name, fn)` → `tool_call_completed` with `tool_call_id = "step:<name>"`.
- `ctx.call(name, …)` → `tool_call_completed` with `tool_call_id = "call:<name>"`.
- `ctx.sleep / sleepUntil` → `effect_observed` with `{ kind: "sleep", … }`.
- `ctx.waitForEvent` → `effect_observed` with `{ kind: "wait", … }`.
- `client.notify` → `effect_observed` with `{ kind: "event", … }`.
- `client.cancel` / `ctx.cancel()` → `effect_observed` with `{ kind: "cancel" }`.

So you get full replay + diff via `@actantdb/replay` and a live event
stream via `@actantdb/studio` for free.

## License

Apache-2.0.
