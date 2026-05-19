import Foundation

/// Lightweight protocol seam for a "spawned subprocess" supervisor. The real
/// implementation (`ActantDBSupervisor`) lives in the `ActantAgent` target,
/// which depends on `ActantDB`; using a protocol here keeps the dependency
/// direction one-way while still letting `Actant.spawned(_:)` accept it.
public protocol SpawnedSupervisor: Sendable {
    /// Start the subprocess if not already started, then return the base URL
    /// at which the spawned `actantdb` server is listening.
    func ensureRunning(dbPath: URL) async throws -> URL
}

/// Construction-time selector for the `Actant` facade. The three modes share
/// the same opaque type; consumers don't branch on platform.
///
/// - `remote`   — `ActantClient` against a hosted `actantdb` server (HTTP/WS).
/// - `spawned`  — `ActantDBSupervisor` (or any `SpawnedSupervisor`) against a
///                child `actantdb` process. Host-only — not available on iOS.
/// - `embedded` — In-process `actant-ffi` bridge. Backs the iOS embedding
///                story; also useful for tests on macOS.
///
/// See `docs/IOS_EMBEDDING.md` and `docs/SYNC_DESIGN.md` for the design
/// rationale and the substrate dependencies.
public enum ActantMode: Sendable {
    case remote(URL)
    #if !os(iOS)
    case spawned(supervisor: any SpawnedSupervisor, dbPath: URL)
    #endif
    case embedded(storeDir: URL, workspaceID: String, actorID: String)
}
