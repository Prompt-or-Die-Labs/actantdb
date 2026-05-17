# 09 — SDK Design

ActantDB ships first-party SDKs for **TypeScript, Python, Swift, and Rust** in Phase 1. Each SDK wraps the same wire protocol (`08-api-spec.md`) and exposes ergonomics that match the host language. This document specifies the shared design — names, shape, error model, subscription handling, codegen — and notes per-language adaptations.

Sections:

1. Shared design principles
2. Client surface
3. Commands
4. Subscriptions
5. Error model
6. Authentication
7. Codegen pipeline
8. TypeScript SDK
9. Python SDK
10. Swift SDK
11. Rust SDK
12. Future SDKs

---

## 1. Shared design principles

1. **One client, one workspace, one actor (by default).** The constructor takes `(base_url, token)` and resolves an `actor_id` / `workspace_id`. Multi-tenant callers explicitly override.
2. **Commands are methods.** `client.command.request_tool_call(...)` is more legible than `client.command("request_tool_call", ...)`. SDKs may keep the generic form as an escape hatch.
3. **Subscriptions return async streams.** The host language's idiomatic stream (AsyncIterable in TS, async generator in Python, `AsyncSequence` in Swift, `Stream` in Rust).
4. **No hidden state.** Calls do not silently retry, cache, or buffer beyond what's documented. Reliability features are explicit options.
5. **Typed from the wire.** Types come from `GET /v1/metadata/commands` via codegen; hand-edits are forbidden in generated files.
6. **No global singletons.** Every SDK supports multiple concurrent client instances.

---

## 2. Client surface

The shape, in pseudo-code:

```
class ActantClient:
  __init__(base_url, token, *, actor_id=None, workspace_id=None,
           timeout=..., transport=..., logger=...)

  // Commands (generated)
  command.create_session(...)
  command.request_tool_call(...)
  // ...

  // Subscriptions
  subscribe(table, filter) -> stream of rows

  // Worker API (optional, gated by capability)
  worker.claim(effect_types, lease_seconds, max_count)
  worker.heartbeat(effect_id, extend_seconds)
  worker.start(effect_id)
  worker.observe(effect_id, observation_ref)

  // Artifacts
  artifacts.upload(bytes, kind, sensitivity) -> { id, uri, hash }
  artifacts.download(id) -> bytes

  // Replay
  replay.start(checkpoint_id, mode, overrides_ref) -> replay_run_id
  replay.get(replay_run_id)

  // Metadata
  metadata.health()
  metadata.version()
```

---

## 3. Commands

The codegen produces a strongly-typed method per command in `03-command-spec.md`. Each method:

- Accepts a typed input object.
- Returns a typed result object plus a list of emitted events.
- Throws / returns `Err` on `status="rejected"`.

Example (TypeScript):

```ts
const result = await client.command.requestToolCall({
  sessionId: "sess_123",
  toolName: "shell.run",
  arguments: { command: "pytest" }
});
// result.toolCallId, result.status, result.approvalRequestId?
```

**Idempotency.** Each command method accepts an optional `idempotencyKey`. If omitted, the SDK generates one and includes it in the request. Callers wishing to enable retries via the same key pass it explicitly.

**Timeouts.** The SDK enforces a per-call deadline. Defaults are conservative for commands (5s) and generous for replays (no default).

---

## 4. Subscriptions

A subscription is an async stream of typed rows. The SDK transparently handles:

- Initial snapshot vs incremental updates (clients see a unified stream of `{type: 'upsert' | 'delete', row}` events, plus `{type: 'snapshot_complete'}`).
- Lag notifications (a `{type: 'lag'}` event followed by re-snapshot).
- Reconnection on transport loss (with exponential backoff and a fresh snapshot).

Example (TypeScript):

```ts
for await (const event of client.subscribe("approval_request", { status: "pending" })) {
  if (event.type === "upsert") {
    renderApproval(event.row);
  } else if (event.type === "delete") {
    removeApproval(event.row_id);
  } else if (event.type === "lag") {
    console.warn("subscription lagged; resyncing");
  }
}
```

**Cancellation.** The stream's iterator handle (e.g. `return()` in JS, `cancel()` in Swift, dropping in Rust) sends `unsubscribe` to the server.

---

## 5. Error model

Two kinds of failures:

- **Transport / protocol** — TLS, DNS, 5xx, 429, malformed responses. Raised as `ActantTransportError` (and language equivalents). May be auto-retried only for safe-by-construction methods (`metadata.*`, `artifacts.download`).
- **Logical** — the server returned `status="rejected"`. Raised as `ActantCommandError` with `code`, `message`, `decision_reason`, `request_id`. Never auto-retried by the SDK.

The error class hierarchy is intentionally shallow — clients pattern-match on `code` (a stable enum) rather than class type.

---

## 6. Authentication

Two constructors:

```
ActantClient.from_bearer(base_url, token)
ActantClient.from_mtls(base_url, cert_path, key_path, ca_path)
```

Token refresh is a client responsibility in Phase 1. Phase 2 adds a `token_provider` callback that the SDK invokes on `401`.

---

## 7. Codegen pipeline

The server publishes its own type catalog at `GET /v1/metadata/commands` and `GET /v1/metadata/tables`. Each SDK has a `codegen` subpackage that:

1. Fetches the catalog from a *reference server* version (pinned in the SDK's metadata, not the user's server).
2. Generates per-language type definitions and the `command.*` method surface.
3. Validates that the user's server version is compatible at runtime (`server.version.schema >= sdk.schema_min`).

**Drift handling.** If the user's server is *newer*, the SDK works (forward-compatible: new commands are unknown to the SDK but the existing ones still type-check). If the server is *older*, the SDK warns or fails per the strictness setting.

---

## 8. TypeScript SDK

Package: `@actantdb/client`

```ts
import { ActantClient } from "@actantdb/client";

const client = new ActantClient({
  baseUrl: "https://actant.example.com",
  token: process.env.ACTANT_TOKEN!,
});

const session = await client.command.createSession({
  agentActorId: "agent_123",
  title: "Fix failing tests",
});

await client.command.appendUserMessage({
  sessionId: session.id,
  text: "Run pytest and report results",
});
```

**Idiomatic features.**

- `async/await` throughout; no callbacks.
- `Result<T, ActantCommandError>` not used; rejections throw.
- `for await ... of client.subscribe(...)` for streams.
- Zero peer dependencies. Bundles a minimal WebSocket polyfill for Node < 21.
- ESM-only (Phase 1); CJS via a tiny shim if requested.

**Build.** `tsc --strict` against TS 5.4+; targets ES2022.

---

## 9. Python SDK

Package: `actantdb`

```python
import asyncio
from actantdb import ActantClient

async def main():
    async with ActantClient(
        base_url="https://actant.example.com",
        token=os.environ["ACTANT_TOKEN"],
    ) as client:
        session = await client.command.create_session(
            agent_actor_id="agent_123",
            title="Fix failing tests",
        )
        async for event in client.subscribe("approval_request", status="pending"):
            print(event)

asyncio.run(main())
```

**Idiomatic features.**

- Async-first; `httpx` for HTTP, `websockets` for WS.
- Sync facade `ActantClient.sync` for scripts that prefer blocking calls. The sync facade runs an event loop internally.
- `Pydantic v2` models for inputs and outputs. Generated.
- Type hints are exhaustive; SDK ships `py.typed`.

**Supported versions.** Python 3.10+.

---

## 10. Swift SDK

Package: `swift-actantdb` (SwiftPM)

```swift
import ActantDB

let client = ActantClient(
    baseURL: URL(string: "https://actant.example.com")!,
    token: ProcessInfo.processInfo.environment["ACTANT_TOKEN"]!
)

let session = try await client.command.createSession(
    agentActorId: "agent_123",
    title: "Fix failing tests"
)

for try await event in client.subscribe(.approvalRequest, where: .status(.pending)) {
    // ...
}
```

**Idiomatic features.**

- `async/await` throughout.
- `AsyncSequence` for subscriptions, cancellable via task cancellation.
- `Codable` types for all inputs/outputs. Generated.
- macOS 14+, iOS 17+, visionOS 1+. Linux supported via Foundation-on-Linux.
- First-class fit for Swoosh and other Apple desktop agents.

---

## 11. Rust SDK

Crate: `actant-client`

```rust
use actant_client::{ActantClient, Config};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ActantClient::new(Config::from_env()?);

    let session = client.command()
        .create_session()
        .agent_actor_id("agent_123")
        .title("Fix failing tests")
        .send()
        .await?;

    let mut sub = client.subscribe()
        .table("approval_request")
        .eq("status", "pending")
        .open()
        .await?;

    while let Some(event) = sub.next().await {
        // ...
    }
    Ok(())
}
```

**Idiomatic features.**

- Builder pattern for command inputs (avoids exploding positional args).
- `Stream` for subscriptions, with `Drop` sending `unsubscribe`.
- `serde` end-to-end. Generated types implement `Serialize + Deserialize`.
- `tokio` runtime by default; an `async-std` feature flag is rejected for Phase 1 to keep the API simple.

**MSRV.** Rust 1.75.

---

## 12. Future SDKs

Phase 2+:

- **Go** — `actant-go`. Channel-based subscriptions; context-based cancellation.
- **Kotlin / Java** — `actant-jvm`. Flow-based subscriptions; coroutines first; Java façade with futures.
- **C#** — `Actant.Client`. `IAsyncEnumerable` for subscriptions; async/await.

The codegen pipeline is shared; each new SDK adds a code emitter and a thin transport.

---

## Verification

- [ ] Every SDK exposes every command in `03-command-spec.md` through a typed method.
- [ ] Every subscription table in `08-api-spec.md` §5 is reachable from every SDK.
- [ ] All four SDKs use the same `code` enum for `ActantCommandError`.
- [ ] The metadata fetch in §7 is the only source of truth — no SDK hand-writes command shapes.
- [ ] No SDK silently retries `command` calls.
