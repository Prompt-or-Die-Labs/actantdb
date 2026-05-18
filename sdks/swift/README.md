# ActantDB Swift SDK

Swift client for an [ActantDB](../../README.md) server. Async/await, `AsyncThrowingStream` for live subscriptions, Foundation-only — no third-party dependencies.

## Install

Add as a SwiftPM dependency:

```swift
.package(path: "../actantDB/sdks/swift"),  // local
// or:
.package(url: "https://github.com/actantdb/swift-actantdb", branch: "main"),
```

```swift
.target(name: "MyApp", dependencies: [
    .product(name: "ActantDB", package: "ActantDB"),
]),
```

Requires **Swift 6.3, macOS 26 / iOS 26**. (The public floor in `planning/sdk-swift.md` is 5.9 / 14; the lift to 6.3 is to match the Swoosh consumer and exercise strict concurrency at build time.)

## Usage

```swift
import ActantDB

let client = ActantClient(
    baseURL: URL(string: "http://127.0.0.1:4555")!,
    token: ProcessInfo.processInfo.environment["ACTANT_TOKEN"]
)

// Health
_ = try await client.healthzReady()

// Commands
let sessionID = try await client.createSession(
    workspaceID: "ws_default",
    actorID: "act_user",
    title: "Fix failing tests"
)

try await client.appendUserMessage(
    workspaceID: "ws_default", actorID: "act_user",
    sessionID: sessionID, text: "Clean up the test artifacts."
)

let resp = try await client.requestToolCall(
    workspaceID: "ws_default", actorID: "act_user",
    sessionID: sessionID,
    toolName: "shell.run",
    arguments: ["cmd": "rm -rf build"]
)

// Approvals
let pending = try await client.approvals(workspaceID: "ws_default")
try await client.approveToolCall(
    workspaceID: "ws_default", actorID: "act_human",
    toolCallID: pending[0].toolCallID, scope: "once"
)

// Replay
let checkpointID = try await client.replayCheckpoint(
    workspaceID: "ws_default", eventID: "evt_xyz"
)
let diff = try await client.replayRun(
    actorID: "act_user",
    checkpointID: checkpointID,
    mode: .memory
)

// Live subscription (AsyncThrowingStream)
let stream = try await client.subscribe(
    workspaceID: "ws_default",
    sessionID: sessionID,
    kind: "events"
)
for try await msg in stream {
    print(msg.json)
}
```

## What's covered

Every endpoint in `actant-server::router`:

| Endpoint                          | Method            |
| --------------------------------- | ----------------- |
| `GET  /v1/healthz` + 3 probes     | `healthz()` etc.  |
| `GET  /v1/metadata/commands`      | `metadataCommands()` |
| `GET  /v1/openapi.yaml`           | `openapi()`       |
| `GET  /v1/metrics`                | `metrics()`       |
| `POST /v1/command`                | `dispatch(...)` + 10 typed convenience methods |
| `GET  /v1/events`                 | `events(sessionID:)` |
| `GET  /v1/approvals`              | `approvals(workspaceID:)` |
| `POST /v1/replay/checkpoint`      | `replayCheckpoint(...)` |
| `POST /v1/replay/run`             | `replayRun(...)`  |
| `POST /v1/sync/since`             | `syncSince(...)`  |
| `GET  /v1/ws`                     | `subscribe(...)`  |

Plus Codable types mirroring `actant-contracts` and `actant-core::model`:

- **Events**: `Sensitivity`, `Risk`, `CausalityKind`, `EventKind`, `ToolCallStatus`, `ActantEvent` (contracts shape — for replay / studio), `AgentEvent` (storage row — for `/v1/events`), `ContextItem`, `ContextManifest`, `ModelCall`, `ToolCallRequest`, `ToolCallCompleted`.
- **Policy**: `Policy`, `ToolRiskEntry`, `ArgDenyRule`, `PolicyVerdict` (custom Codable for internally-tagged union), `ApprovalRequest`, `ApprovalDecisionV` (custom Codable), `PendingApproval`.
- **Replay**: `CheckpointRef`, `ReplayOverrides`, `ReplayRun`, `DiffKind`, `DiffEntry`, `ReplayDiff`, `ReplayMode`.
- **Commands**: `CommandType` (enum of every alpha command), `CommandRequest`, `CommandResponse`, plus per-endpoint response shells.
- **`JSONValue`** — Codable enum representing arbitrary JSON, plus `Any` bridges and `ExpressibleBy*` literals.

## Errors

`ActantError` covers HTTP, transport, decoding, WebSocket, and cancellation. The `http` case carries the server's typed error kind:

```swift
do {
    _ = try await client.dispatch(...)
} catch let ActantError.http(status, kind, message, _) {
    switch kind {
    case "approval_required":  /* 202 — queue for human review */
    case "rate_limited":       /* 429 — back off, see retry-after header */
    case "idempotent_replay":  /* 200 — same idempotency_key, no new event */
    case "permission_denied", "approval_denied", "invalid_token", "workspace_mismatch":
        /* 401/403 */
    case "not_found":          /* 404 */
    default: break
    }
}
```

Note: the client inspects the body for a top-level `error` field on every response, so a 200 with `idempotent_replay` and a 202 with `approval_required` both surface as `ActantError.http` rather than silently parsing as a successful `CommandResponse`.

## Auth

`StaticToken` covers the v0 case — pass a pre-minted HS256 JWT via the `token:` initializer. The server's auth path enforces `iss == workspace_id`. OIDC discovery / JWKS refresh is on the server but not yet on the client; implement `TokenProvider` for refresh flows when needed.

## Build / test

```bash
swift build
swift test
```

25 tests, ~0.03s, zero external dependencies. Tests use `MockURLProtocol` with an actor-based cross-suite mutex so handler state never races.

## Status

Hand-written. Codegen-from-contracts (`cargo run -p actant-contracts -- codegen-swift`) is a follow-up; until then, keep these types in sync with `crates/actant-contracts/src/{events,policy,replay}.rs` and `crates/actant-core/src/model.rs`.
