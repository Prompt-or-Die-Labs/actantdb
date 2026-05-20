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

Most tests use `MockURLProtocol` with an actor-based cross-suite mutex so handler state never races.

To validate the embedded path before a published `ActantFFI.xcframework` exists:

```bash
bash sdks/swift/scripts/build-local-actantffi-xcframework.sh
ACTANTDB_LOCAL_FFI_XCFRAMEWORK=".actantffi/ActantFFI.xcframework" \
  swift test --package-path sdks/swift --filter embeddedRoundTrip
```

## ActantAgent (opinionated facade)

Sitting on top of `ActantDB` is a second library product, `ActantAgent`,
that exposes a smaller, opinionated surface. The shape is chosen so a
consumer can wire ActantDB into its own types via one-line conformance
extensions instead of writing adapters. The facade is **generic over the
consumer's data shapes** — the SDK never needs to know what
`YourCore.ChatMessage` is.

```swift
import ActantAgent

let supervisor = ActantDBSupervisor()           // spawns + lifecycles the
                                                // actantdb Rust subprocess
let url = try await supervisor.start(
    dbPath: URL(fileURLWithPath: "/path/to/db")
)

let backend = AgentBackend(
    client: ActantClient(baseURL: url),
    workspaceID: "ws_default",
    actorID: "act_user"
)
try await backend.waitForReady(timeout: 5)

// Session, generic over your message type.
let session = Session<YourCore.ChatMessage>(
    backend: backend,
    sessionID: "sess_123",
    encode: { ($0.role.toActant, $0.text) },
    decode: { role, text, _ in YourCore.ChatMessage(role: .init(role), text: text) }
)
try await session.appendMessage(myMessage)
let transcript = try await session.loadTranscript()

// Governed memory.
let memory = MemoryStore(backend: backend)
let candidateID = try await memory.propose(
    text: "User prefers dark mode", category: "preference",
    sensitivity: .low, confidence: 0.9, evidence: .null
)
try await memory.approve(candidateID: candidateID)
let approved = try await memory.listApproved()      // [ApprovedMemory]
let pending  = try await memory.listPending()       // [MemoryCandidate]
let clashes  = try await memory.listConflicts()     // [MemoryConflict]

// Audit, generic over your record type.
struct DecisionRecord: Codable, Sendable { let action: String; let why: String }
let auditor = Auditor<DecisionRecord>(
    backend: backend, sessionID: "sess_123", sentinelKey: "swoosh.decision"
)
try await auditor.log(DecisionRecord(action: "skip", why: "user paused"))
let last = try await auditor.last()                  // DecisionRecord?

// Approvals.
let approvals = ApprovalCenter(backend: backend)
let pending2 = try await approvals.pending()
try await approvals.approve(toolCallID: pending2[0].toolCallID, scope: "once")

// Relationships — small directed knowledge graph.
let rels = RelationshipStore(backend: backend)
let alice = try await rels.upsertEntity(type: "person", canonicalName: "Alice")
let bob   = try await rels.upsertEntity(type: "person", canonicalName: "Bob")
_ = try await rels.link(source: alice, relation: "knows", target: bob, confidence: 0.9)
let neighbors = try await rels.neighbors(of: bob)    // [EntityRelationRow]

// Replay.
let replay = ReplayClient(backend: backend)
let cp = try await replay.checkpoint(eventID: "evt_xyz")
let diff = try await replay.run(actorID: "act_user", checkpointID: cp, mode: .memory)
```

The intended consumer-side pattern is to extend the facade types with
your own protocol conformances:

```swift
extension ActantAgent.Session: SwooshCore.TranscriptStore {}
extension ActantAgent.MemoryStore: SwooshCore.GovernedMemory {}
extension ActantAgent.ApprovalCenter: SwooshCore.ApprovalQueue {}
extension ActantAgent.RelationshipStore: SwooshCore.KnowledgeGraph {}
```

No adapter classes, no wrapper layer.

### Runtime state

`FileBackedRuntimeStateStore` persists goals, manifestation history, scout
state, and workflow drafts as a JSON snapshot. Point it at an Application
Support file and construct a new store with the same URL after a daemon restart.

```swift
let stateURL = appSupport.appending(path: "runtime-state.json")
let state = FileBackedRuntimeStateStore(fileURL: stateURL)
try await state.upsertGoal(RuntimeGoal(
    id: "goal_1",
    title: "Ship local persistence",
    createdAt: now,
    updatedAt: now
))
let snapshot = try await state.load()
```

### Subprocess supervisor

`ActantDBSupervisor` spawns and lifecycles the `actantdb` Rust binary for
local-first deployments. Binary discovery order: `binaryPath` argument →
`SWOOSH_ACTANTDB_PATH` env → `PATH` → `~/.cargo/bin/actantdb` →
`extraSearchPaths` (consumer-supplied, e.g. `Bundle.main.resourceURL`). If
nothing matches, throws `Error.binaryNotFound` whose
`localizedDescription` carries the exact `cargo install` command.

```swift
let sup = ActantDBSupervisor(
    binaryPath: nil,
    extraSearchPaths: [Bundle.main.resourceURL!],
    logOutputTo: URL(fileURLWithPath: "/tmp/actantdb.log")
)
let url = try await sup.start(dbPath: ..., port: nil)   // ephemeral port
defer { Task { await sup.stop() } }                     // SIGTERM, then SIGKILL after 10s
```

## Status

Hand-written. Codegen-from-contracts (`cargo run -p actant-contracts -- codegen-swift`) is a follow-up; until then, keep these types in sync with `crates/actant-contracts/src/{events,policy,replay}.rs` and `crates/actant-core/src/model.rs`.
