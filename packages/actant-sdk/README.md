# @actantdb/sdk

TypeScript client for the ActantDB HTTP+WS server.

## Install

```bash
npm install @actantdb/sdk @actantdb/types
```

## Use

```ts
import { ActantClient } from "@actantdb/sdk";

const client = new ActantClient({ baseUrl: "http://127.0.0.1:4555" });

// Health check
const h = await client.healthz();

// The alpha command flow
const { sessionId } = await client.createSession({
  workspaceId: "ws_default",
  actorId: "act_system",
});

await client.appendUserMessage({
  workspaceId: "ws_default",
  actorId: "act_system",
  sessionId,
  text: "Fix the failing tests.",
});

const { toolCallId, status, verdict } = await client.requestToolCall({
  workspaceId: "ws_default",
  actorId: "act_system",
  sessionId,
  toolName: "shell.run",
  arguments: { command: "pytest -q" },
});

if (status === "pending_approval") {
  await client.approveToolCall({
    workspaceId: "ws_default",
    actorId: "act_system",
    toolCallId,
  });
}

await client.recordToolResult({
  workspaceId: "ws_default",
  actorId: "act_system",
  toolCallId,
  result: { stdout: "1 passed", exit: 0 },
});

// Print the Chronicle
const { events } = await client.events({ sessionId });
console.log(events.map((e) => e.event_type));
```

## Live subscription

```ts
const ws = client.subscribe({ workspaceId: "ws_default", kind: "events" });
ws.onmessage = (m) => console.log(JSON.parse(m.data));
```
