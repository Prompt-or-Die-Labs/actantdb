import Foundation

// On Apple platforms CloudKit is available and `Actant.sync` returns a
// `CloudKitSync` instance; on Linux / Windows / wasm callers get this stub
// so the public surface stays identical. Every method short-circuits with
// `SyncError.unsupportedPlatform`.
#if !canImport(CloudKit) || os(Linux)

public final class NoOpSync: ActantSync {
    public init() {}

    public func enable(container: String, options: SyncOptions) async throws {
        _ = (container, options)
        throw SyncError.unsupportedPlatform
    }

    public func disable() async throws {
        throw SyncError.unsupportedPlatform
    }

    public func status() async throws -> SyncStatus {
        throw SyncError.unsupportedPlatform
    }
}

#endif
