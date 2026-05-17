# Work package: `sdks/swift` — `swift-actantdb`

## Context

Swift SDK. Phase 1 if Swoosh (Mac product) needs it; otherwise Phase 6 in lockstep with the other SDKs.

## Specs to read first

- `/specs/09-sdk-design.md` §10.
- `/planning/sdk-swift.md`.

## Scope

### Layout

```
sdks/swift/
├── Package.swift               (macOS 14+, iOS 17+, visionOS 1+, Linux)
├── README.md
├── Sources/ActantDB/
│   ├── ActantClient.swift
│   ├── Transport.swift
│   ├── Subscribe.swift         (AsyncSequence)
│   ├── Errors.swift
│   ├── Auth.swift
│   └── Generated/              (codegen output)
└── Tests/ActantDBTests/
```

### Tests

- `swift test` on macOS + Linux runners.
- Subscription cancellation via task cancellation cleanly closes the websocket.
- Decodable round-trip for every Generated type.

## Acceptance criteria

- [ ] `swift build && swift test` green on macOS + Linux.
- [ ] No `#warning` / `#error` directives.
- [ ] Compatible with Linux's Foundation port for cloud-side usage.

## Do NOT

- Do NOT depend on Apple-only APIs in cross-platform code.
- Do NOT vend a CocoaPods spec. SwiftPM only.

## Hand-off

`swift test` plus a SwiftUI smoke app integration test.
