# SDK design — Swift

Package: `swift-actantdb` (SwiftPM).

## Tech

- Swift 5.9+. SwiftPM only — no CocoaPods.
- async/await throughout. `AsyncSequence` for subscriptions.
- `Codable` types for all inputs/outputs. Generated.
- macOS 14+, iOS 17+, visionOS 1+. Foundation-on-Linux supported.

## API

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
