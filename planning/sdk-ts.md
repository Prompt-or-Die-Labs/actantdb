# SDK design — TypeScript

Package: `@actantdb/client`. Phase 1 ships full surface for the alpha command set; subsequent phases regenerate as the metadata grows.

## Tech

- Pure ESM, TypeScript 5.4+, target ES2022.
- Zero peer dependencies.
- WebSocket polyfill bundled for Node < 21; native WebSocket on browsers + Node ≥ 21.
- No bundler in the published package; users bundle.

## API

```ts
import { ActantClient } from "@actantdb/client";

const client = new ActantClient({
  baseUrl: "https://actant.example.com",
  token: process.env.ACTANT_TOKEN!,
});

// Generated typed commands
const session = await client.command.createSession({
  agentActorId: "agent_123",
  title: "Fix failing tests",
});

await client.command.appendUserMessage({
  sessionId: session.id,
  text: "Run pytest and report results",
});

// Subscriptions
for await (const event of client.subscribe("approval_request", { status: "pending" })) {
  if (event.type === "upsert") renderApproval(event.row);
  else if (event.type === "delete") removeApproval(event.row_id);
  else if (event.type === "lag") console.warn("subscription lagged");
}
```

## Distribution

- Published to npm under `@actantdb`.
- Source under `sdks/ts/`.
- Generated code under `sdks/ts/src/generated/` — never hand-edited.
- Codegen from `actant-sdk-codegen --target ts --out sdks/ts/src/generated/`.

## Versioning

`MAJOR.MINOR.PATCH` aligned with `actantdb-server`'s schema:

- `client.0.x` works against server schema 1.
- Schema 2 ships `client.1.x`.
- Within a major, the client is **forward-compatible** with newer servers (unknown commands are still typed in older clients only after a regen).
