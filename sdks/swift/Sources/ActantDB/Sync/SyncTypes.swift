import Foundation

// MARK: - Cross-platform sync API surface
//
// `SyncOptions`, `SyncStatus`, `SyncError`, and the `ActantSync` protocol are
// always available — `Actant.sync` returns the platform-appropriate
// implementation (`CloudKitSync` on Apple, `NoOpSync` elsewhere) without
// callers branching on platform.

public struct SyncOptions: Sendable {
    /// CloudKit-side TTL applied to ledger records. Older records are pruned
    /// from the cloud copy; the local ledger keeps everything indefinitely.
    public var retainDays: Int
    /// When true, the SDK re-arms its CloudKit push subscriptions on app
    /// foreground (`appWillEnterForeground` on iOS). When false, syncing only
    /// happens when callers explicitly call `status()` or write an event.
    public var pushOnAppActive: Bool

    public init(retainDays: Int = 30, pushOnAppActive: Bool = true) {
        self.retainDays = retainDays
        self.pushOnAppActive = pushOnAppActive
    }
}

public struct SyncStatus: Sendable {
    /// HLC cursor of the most recent event the device has acknowledged from
    /// CloudKit. `nil` before the first successful pull.
    public let syncedAtHLC: HLCCursor?
    /// Number of locally-authored events still waiting to be uploaded.
    public let outboxQueueDepth: Int
    /// Wall-clock time of the most recent inbound CloudKit record. `nil`
    /// before the first push wakes the device.
    public let lastInboundEventAt: Date?
    /// True when a CKQuerySubscription is currently registered with CloudKit.
    public let activeSubscription: Bool

    public init(
        syncedAtHLC: HLCCursor? = nil,
        outboxQueueDepth: Int = 0,
        lastInboundEventAt: Date? = nil,
        activeSubscription: Bool = false
    ) {
        self.syncedAtHLC = syncedAtHLC
        self.outboxQueueDepth = outboxQueueDepth
        self.lastInboundEventAt = lastInboundEventAt
        self.activeSubscription = activeSubscription
    }
}

public enum SyncError: Error, Sendable, CustomStringConvertible {
    /// Returned by `NoOpSync` on Linux / Windows / wasm. Apple platforms get
    /// the real `CloudKitSync`.
    case unsupportedPlatform
    /// CloudKit returned a non-recoverable error (account disabled,
    /// container not provisioned, etc.). The string carries the original
    /// description.
    case cloudKit(String)
    /// The caller asked for an operation that depends on `enable()` having
    /// succeeded, but sync is still disabled.
    case notEnabled
    /// FFI/storage error from the underlying `Actant`.
    case storage(String)

    public var description: String {
        switch self {
        case .unsupportedPlatform:
            return "SyncError.unsupportedPlatform — CloudKit sync requires an Apple platform"
        case .cloudKit(let msg):
            return "SyncError.cloudKit: \(msg)"
        case .notEnabled:
            return "SyncError.notEnabled — call sync.enable(container:) first"
        case .storage(let msg):
            return "SyncError.storage: \(msg)"
        }
    }
}

/// Sync facade returned by `Actant.sync`. `CloudKitSync` (Apple) and
/// `NoOpSync` (elsewhere) both conform; the surface is identical so consumer
/// code is platform-agnostic.
public protocol ActantSync: Sendable {
    /// Enable CloudKit sync against the given iCloud container. On non-Apple
    /// platforms this throws `SyncError.unsupportedPlatform`.
    func enable(container: String, options: SyncOptions) async throws

    /// Tear down the active subscription and drainer task. Local ledger state
    /// is preserved; on next `enable(container:)` the device re-subscribes
    /// and resumes from its persisted cursor.
    func disable() async throws

    /// Snapshot of the current sync state. Cheap; safe to poll from UI.
    func status() async throws -> SyncStatus
}
