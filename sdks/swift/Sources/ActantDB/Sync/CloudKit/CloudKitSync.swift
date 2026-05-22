#if canImport(CloudKit) && !os(Linux)
import Foundation
import CloudKit

/// CloudKit-backed implementation of `ActantSync`. Owns the
/// `OutboxDrainer` and `SubscriptionManager` instances; serializes state
/// access through an internal `actor` so the public surface remains a
/// `Sendable final class`.
///
/// See `docs/SYNC_DESIGN.md` for the replication shape (one EventRow per
/// CKRecord, HLC cursor, push-subscription resume) and the failure modes
/// this implementation has to handle.
///
public final class CloudKitSync: ActantSync {
    private let actant: Actant
    private let containerID: String
    private let options: SyncOptions
    private let state: StateActor

    public init(actant: Actant, container: String, options: SyncOptions = SyncOptions()) {
        self.actant = actant
        self.containerID = container
        self.options = options
        self.state = StateActor()
    }

    public func enable(container: String, options: SyncOptions) async throws {
        // Allow re-enable with a different container or option set.
        let ckContainer = CKContainer(identifier: container)
        let database = ckContainer.privateCloudDatabase
        let subs = SubscriptionManager(database: database, containerID: container)
        let outbox: CloudKitOutboxStore
        do {
            outbox = try CloudKitOutboxStore.defaultStore(containerID: container)
        } catch {
            throw SyncError.storage(error.localizedDescription)
        }
        let drainer = OutboxDrainer(
            actant: actant,
            database: database,
            options: options,
            outbox: outbox
        )

        do {
            try await subs.registerIfNeeded()
        } catch {
            throw SyncError.cloudKit(error.localizedDescription)
        }
        await drainer.start()

        await state.setEnabled(
            subs: subs,
            drainer: drainer,
            container: container,
            options: options
        )
    }

    public func disable() async throws {
        guard let (subs, drainer) = await state.takeIfEnabled() else {
            throw SyncError.notEnabled
        }
        await drainer.stop()
        try? await subs.unregister()
    }

    public func status() async throws -> SyncStatus {
        guard let snapshot = await state.snapshot() else {
            return SyncStatus(
                syncedAtHLC: nil,
                outboxQueueDepth: 0,
                lastInboundEventAt: nil,
                activeSubscription: false
            )
        }
        let depth = await snapshot.drainer.queueDepth()
        let cursor = await snapshot.drainer.lastSyncedHLC()
        let lastIn = await snapshot.drainer.lastInbound()
        let active = await snapshot.subs.isActive()
        return SyncStatus(
            syncedAtHLC: cursor,
            outboxQueueDepth: depth,
            lastInboundEventAt: lastIn,
            activeSubscription: active
        )
    }

    // MARK: - Internal state actor

    private actor StateActor {
        private var subs: SubscriptionManager?
        private var drainer: OutboxDrainer?
        private var containerID: String?
        private var options: SyncOptions?

        struct Snapshot {
            let subs: SubscriptionManager
            let drainer: OutboxDrainer
        }

        func setEnabled(
            subs: SubscriptionManager,
            drainer: OutboxDrainer,
            container: String,
            options: SyncOptions
        ) {
            self.subs = subs
            self.drainer = drainer
            self.containerID = container
            self.options = options
        }

        func takeIfEnabled() -> (SubscriptionManager, OutboxDrainer)? {
            guard let s = subs, let d = drainer else { return nil }
            subs = nil
            drainer = nil
            containerID = nil
            options = nil
            return (s, d)
        }

        func snapshot() -> Snapshot? {
            guard let s = subs, let d = drainer else { return nil }
            return Snapshot(subs: s, drainer: d)
        }
    }
}

#endif // canImport(CloudKit) && !os(Linux)
