import Foundation

/// Backing protocol for `Actant` when running in `.embedded` mode. The
/// production implementation (`ActantFFIBridge`) proxies into the
/// uniffi-generated `ActantHandle` Swift class shipped by `ActantFFI.xcframework`.
///
/// Lives behind a protocol so:
///   - Tests can stub the bridge without spinning up the Rust core.
///   - The package compiles cleanly on platforms / build configurations where
///     the `ActantFFI` binary target is missing — see `ActantFFIBridge.swift`
///     for the `#if canImport(ActantFFI)` guard.
///
/// Method signatures mirror `ActantHandle` (see `docs/IOS_EMBEDDING.md` §1)
/// rather than `ActantClient` so the FFI path is a thin one-to-one proxy.
public protocol EmbeddedHandle: Sendable {
    func dispatch(
        commandType: String,
        input: JSONValue,
        idempotencyKey: String?
    ) async throws -> CommandOutcome

    func eventsSince(
        cursor: HLCCursor?,
        limit: UInt32
    ) async throws -> [EventRow]

    func ingest(
        events: [EventRow]
    ) async throws -> IngestReport

    func close() async
}
