# SDK design — Swift

Package: `swift-actantdb` (SwiftPM).

## Tech

- Swift 5.9+. SwiftPM only — no CocoaPods.
- async/await throughout. `AsyncSequence` for subscriptions.
- `Codable` types for all inputs/outputs. Generated.
- macOS 14+, iOS 17+, visionOS 1+. Foundation-on-Linux supported.

## Two-tier API

The SDK ships two modules. Consumers pick the level that matches the
shape of the code they already have.

### Low tier — `ActantDB`

Direct HTTP/WS client. Mirrors `actant-server::router` 1:1. Use when you
already have your own session/memory/approval abstractions and just want a
wire client.

```swift
import ActantDB

let client = ActantClient(
    baseURL: URL(string: "https://actant.example.com")!,
    token: ProcessInfo.processInfo.environment["ACTANT_TOKEN"]!
)

let sessionID = try await client.createSession(
    workspaceID: "ws_default", actorID: "act_user", title: "Fix failing tests"
)

for try await msg in try await client.subscribe(
    workspaceID: "ws_default", sessionID: sessionID, kind: "events"
) {
    // ...
}
```

### High tier — `ActantAgent` (opinionated facade)

Use when you're building a Swift agent and want to add ActantDB by
extending your own types with one-line conformances rather than wiring
adapters. The facade is **generic over the consumer's data shapes** so
the SDK never has to know what `YourCore.ChatMessage` is.

```swift
import ActantAgent

let supervisor = ActantDBSupervisor()
let url = try await supervisor.start(
    dbPath: URL(fileURLWithPath: "~/Library/Application Support/swoosh/actantdb")
)

let backend = AgentBackend(
    client: ActantClient(baseURL: url),
    workspaceID: "ws_default",
    actorID: "act_user"
)

// Consumer plugs in its own ChatMessage shape — no adapter needed.
let session = Session<SwooshCore.ChatMessage>(
    backend: backend,
    sessionID: "sess_123",
    encode: { ($0.role.toActant, $0.text) },
    decode: { role, text, _ in SwooshCore.ChatMessage(role: .init(role), text: text) }
)
try await session.appendMessage(myMessage)

let memory = MemoryStore(backend: backend)
let approved = try await memory.listApproved()

let relations = RelationshipStore(backend: backend)
let alice = try await relations.upsertEntity(type: "person", canonicalName: "Alice")
let bob   = try await relations.upsertEntity(type: "person", canonicalName: "Bob")
_ = try await relations.link(source: alice, relation: "knows", target: bob)
```

The five other facade types — `Auditor<Record>`, `ApprovalCenter`,
`ReplayClient`, `RelationshipStore`, `ActantDBSupervisor` — follow the same
shape. See `sdks/swift/README.md` for the full surface.

## Distribution

- Published under github.com/actantdb/swift-actantdb (SwiftPM).
- Source under `sdks/swift/`.
- Generated code under `sdks/swift/Sources/ActantDB/Generated/`.
- Codegen from `actant-sdk-codegen --target swift --out sdks/swift/Sources/ActantDB/Generated/`.

## Conventions

- camelCase Swift idioms; the codegen translates from JSON schema.
- Concurrency cancellation: `AsyncSequence`'s task cancellation cleanly cancels the underlying subscription.
- WebSocket: `URLSessionWebSocketTask` on Apple platforms; `swift-nio` on Linux.

## Versioning

Same schema major as TS/Python.

## Why Swift is first-class

ActantDB powers Swoosh (a personal Mac agent product). The Swift SDK is the path to product use on Apple devices and is treated as a first-class SDK, not an afterthought.
