import Foundation

/// Unified construction layer over the three transports the SDK supports —
/// `.remote(url:)`, `.spawned(supervisor:)`, and `.embedded(storeDir:)`.
/// Consumers code against `Actant`; the transport is a config choice rather
/// than a code branch.
///
/// API rationale:
///   - `workspaceID` / `actorID` live on the facade itself (not on every
///     `dispatch` call) so the three constructors converge on a single
///     surface and embedded mode's eager workspace binding maps cleanly.
///   - The Sync facade hangs off `actant.sync`. On Apple platforms it's a
///     `CloudKitSync`; elsewhere a `NoOpSync` that throws
///     `SyncError.unsupportedPlatform`. Either way the call site is the same.
///
/// See `docs/IOS_EMBEDDING.md` for the embedded-mode design and
/// `docs/SYNC_DESIGN.md` for the sync shape.
public actor Actant {
    public let mode: ActantMode
    public let workspaceID: String
    public let actorID: String

    private let remoteClient: ActantClient?
    private let embedded: (any EmbeddedHandle)?

    /// Sync facade. Cross-platform — call `actant.sync.enable(container:options:)`
    /// from any platform; non-Apple builds get a stub that throws
    /// `SyncError.unsupportedPlatform`.
    public nonisolated let sync: any ActantSync

    // MARK: - Construction

    internal init(
        mode: ActantMode,
        workspaceID: String,
        actorID: String,
        remoteClient: ActantClient?,
        embedded: (any EmbeddedHandle)?,
        sync: any ActantSync
    ) {
        self.mode = mode
        self.workspaceID = workspaceID
        self.actorID = actorID
        self.remoteClient = remoteClient
        self.embedded = embedded
        self.sync = sync
    }

    /// Construct a remote-mode `Actant`. Backs every call with `ActantClient`
    /// against the supplied HTTP base URL.
    public static func remote(
        _ url: URL,
        workspaceID: String,
        actorID: String,
        token: String? = nil
    ) async throws -> Actant {
        let client = ActantClient(baseURL: url, token: token)
        let sync = Self.makeDefaultSync()
        let actant = Actant(
            mode: .remote(url),
            workspaceID: workspaceID,
            actorID: actorID,
            remoteClient: client,
            embedded: nil,
            sync: sync
        )
        Self.bindSync(sync, owner: actant)
        return actant
    }

    #if !os(iOS)
    /// Construct a spawned-mode `Actant`. The supervisor is asked to start
    /// (idempotent) and the returned base URL is wrapped in an `ActantClient`.
    /// Not available on iOS — the supervisor type is gated out of the build.
    public static func spawned(
        _ supervisor: any SpawnedSupervisor,
        dbPath: URL,
        workspaceID: String,
        actorID: String
    ) async throws -> Actant {
        let baseURL = try await supervisor.ensureRunning(dbPath: dbPath)
        let client = ActantClient(baseURL: baseURL)
        let sync = Self.makeDefaultSync()
        let actant = Actant(
            mode: .spawned(supervisor: supervisor, dbPath: dbPath),
            workspaceID: workspaceID,
            actorID: actorID,
            remoteClient: client,
            embedded: nil,
            sync: sync
        )
        Self.bindSync(sync, owner: actant)
        return actant
    }
    #endif

    /// Construct an embedded-mode `Actant`. Uses the in-process `actant-ffi`
    /// bridge against the supplied store directory.
    ///
    /// When the `ActantFFI` binary target is not linked (e.g. local
    /// `swift build` before the first XCFramework release), this throws an
    /// `ActantError.transport` describing the missing dependency.
    public static func embedded(
        storeDir: URL,
        workspaceID: String,
        actorID: String
    ) async throws -> Actant {
        let bridge = try ActantFFIBridge(
            storeDir: storeDir,
            workspaceID: workspaceID,
            actorID: actorID
        )
        let sync = Self.makeDefaultSync()
        let actant = Actant(
            mode: .embedded(storeDir: storeDir, workspaceID: workspaceID, actorID: actorID),
            workspaceID: workspaceID,
            actorID: actorID,
            remoteClient: nil,
            embedded: bridge,
            sync: sync
        )
        Self.bindSync(sync, owner: actant)
        return actant
    }

    // MARK: - Unified API
    //
    // Both backends converge on the FFI shape (`CommandOutcome` / `EventRow` /
    // `IngestReport`); the remote path adapts `ActantClient`'s HTTP wire
    // shape into those types.

    public func dispatch(
        command: String,
        input: JSONValue,
        idempotencyKey: String? = nil
    ) async throws -> CommandOutcome {
        if let embedded {
            return try await embedded.dispatch(
                commandType: command,
                input: input,
                idempotencyKey: idempotencyKey
            )
        }
        guard let remoteClient else {
            throw ActantError.transport("Actant has no transport configured")
        }
        let resp = try await remoteClient.dispatch(
            workspaceID: workspaceID,
            actorID: actorID,
            commandType: command,
            input: input,
            idempotencyKey: idempotencyKey
        )
        return CommandOutcome(
            commandID: resp.commandID,
            eventID: resp.eventID,
            result: resp.result
        )
    }

    public func eventsSince(
        _ cursor: HLCCursor?,
        limit: Int = 200
    ) async throws -> [EventRow] {
        let clampedLimit = UInt32(max(1, min(limit, Int(UInt32.max))))
        if let embedded {
            return try await embedded.eventsSince(cursor: cursor, limit: clampedLimit)
        }
        guard let remoteClient else {
            throw ActantError.transport("Actant has no transport configured")
        }
        // Remote path: the existing `/v1/sync/since` endpoint is event-id
        // cursored rather than HLC-cursored. Until the substrate exposes an
        // HLC-aware endpoint, fall back to the legacy id-based cursor; if
        // the caller passed an HLCCursor we use its `physicalMS:logical` as
        // an opaque token (the server treats unknown cursors as "from
        // beginning"). TODO: switch to a /v1/events/since-hlc endpoint when
        // it lands.
        let token = cursor.map { "\($0.physicalMS):\($0.logical)" } ?? ""
        let resp = try await remoteClient.syncSince(
            workspaceID: workspaceID,
            sinceEventID: token,
            limit: clampedLimit
        )
        return resp.events.map { syncEvent in
            EventRow(
                id: syncEvent.id,
                workspaceID: self.workspaceID,
                sessionID: nil,
                deviceID: "_remote_",
                actorID: syncEvent.actorID,
                eventType: syncEvent.eventType,
                payloadJSON: syncEvent.payloadInline?.data(using: .utf8) ?? Data(),
                payloadHash: syncEvent.payloadHash,
                prevChainHash: nil,
                hlc: cursor ?? HLCCursor(physicalMS: 0, logical: 0),
                createdAt: syncEvent.createdAt
            )
        }
    }

    public func ingest(_ events: [EventRow]) async throws -> IngestReport {
        if let embedded {
            return try await embedded.ingest(events: events)
        }
        // Remote path: no public ingest endpoint today — the HTTP server is
        // the canonical writer. Surface the gap rather than silently dropping
        // the batch.
        throw ActantError.transport(
            "ingest(_:) is only supported in embedded mode; remote ingest endpoint pending substrate work"
        )
    }

    /// Convenience accessor — returns the underlying `ActantClient` when the
    /// mode uses one (remote or spawned). Useful for consumers that want to
    /// reach endpoints the unified surface doesn't expose yet.
    public func underlyingClient() -> ActantClient? {
        return remoteClient
    }

    // MARK: - Sync factory

    private static func makeDefaultSync() -> any ActantSync {
        #if canImport(CloudKit) && !os(Linux)
        // CloudKit instance is created lazily by the deferred wrapper on the
        // first `enable(container:)` call — that keeps `Actant.init` free of
        // back-reference cycles.
        return DeferredCloudKitSync()
        #else
        return NoOpSync()
        #endif
    }

    private static func bindSync(_ sync: any ActantSync, owner: Actant) {
        #if canImport(CloudKit) && !os(Linux)
        (sync as? DeferredCloudKitSync)?.bind(owner: owner)
        #endif
    }
}

// MARK: - Testing shim
//
// Tests need to inject an `ActantClient` built against a custom URLSession
// (MockURLProtocol). The public `Actant.remote(_:)` uses `URLSession.shared`;
// this shim exposes the underlying construction path with an explicit client.
// `internal` visibility — only reachable from `@testable import ActantDB`.

enum ActantTestingShim {
    static func make(
        client: ActantClient,
        workspaceID: String,
        actorID: String
    ) async throws -> Actant {
        let sync = Actant.makeDefaultSyncForTesting()
        let actant = Actant(
            mode: .remote(client.baseURL),
            workspaceID: workspaceID,
            actorID: actorID,
            remoteClient: client,
            embedded: nil,
            sync: sync
        )
        Actant.bindSyncForTesting(sync, owner: actant)
        return actant
    }
}

extension Actant {
    static func makeDefaultSyncForTesting() -> any ActantSync {
        #if canImport(CloudKit) && !os(Linux)
        return DeferredCloudKitSync()
        #else
        return NoOpSync()
        #endif
    }

    static func bindSyncForTesting(_ sync: any ActantSync, owner: Actant) {
        #if canImport(CloudKit) && !os(Linux)
        (sync as? DeferredCloudKitSync)?.bind(owner: owner)
        #endif
    }
}

#if canImport(CloudKit) && !os(Linux)

/// Lazy `ActantSync` wrapper that defers CloudKit instantiation until the
/// first `enable(container:options:)` call. Required because `Actant` and the
/// sync facade have a mutual dependency (sync calls back into `actant.ingest`).
final class DeferredCloudKitSync: ActantSync, @unchecked Sendable {
    private let lock = NSLock()
    private var inner: CloudKitSync?
    private weak var owner: Actant?

    func bind(owner: Actant) {
        lock.withLock { self.owner = owner }
    }

    func enable(container: String, options: SyncOptions) async throws {
        // Bound `owner` is required for the CloudKit→ingest callback path.
        // Tests can call `Actant.remote(...)` without ever touching sync, so
        // we only enforce the binding here.
        let actant = lock.withLock { self.owner }
        guard let actant else {
            throw SyncError.notEnabled
        }
        let real = lock.withLock { () -> CloudKitSync in
            if let existing = inner { return existing }
            let new = CloudKitSync(actant: actant, container: container, options: options)
            inner = new
            return new
        }
        try await real.enable(container: container, options: options)
    }

    func disable() async throws {
        let real = lock.withLock { inner }
        guard let real else { throw SyncError.notEnabled }
        try await real.disable()
    }

    func status() async throws -> SyncStatus {
        let real = lock.withLock { inner }
        guard let real else {
            return SyncStatus()
        }
        return try await real.status()
    }
}

#endif
