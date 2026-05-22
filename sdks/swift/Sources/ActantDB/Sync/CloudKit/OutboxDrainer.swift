#if canImport(CloudKit) && !os(Linux)
import Foundation
import CloudKit

/// Periodically drains the local outbox into CloudKit and pulls any inbound
/// records that arrived since the last sync cursor. Retries via a circuit
/// breaker shape modeled on `actant-reliability::circuit` — failures bump a
/// backoff counter; success resets it.
actor OutboxDrainer {
    private let actant: Actant
    private let database: CKDatabase
    private let options: SyncOptions
    private let outbox: CloudKitOutboxStore

    private var lastSynced: HLCCursor?
    private var lastInboundAt: Date?
    private var drainTask: Task<Void, Never>?
    private var consecutiveFailures: Int = 0

    private let zoneID = CKRecordZone.ID(zoneName: "ActantDBLedger", ownerName: CKCurrentUserDefaultName)

    init(
        actant: Actant,
        database: CKDatabase,
        options: SyncOptions,
        outbox: CloudKitOutboxStore
    ) {
        self.actant = actant
        self.database = database
        self.options = options
        self.outbox = outbox
    }

    func start() {
        guard drainTask == nil else { return }
        drainTask = Task { [weak self] in
            await self?.runLoop()
        }
    }

    func stop() async {
        drainTask?.cancel()
        drainTask = nil
    }

    func enqueue(_ events: [EventRow]) async throws {
        try await outbox.append(events)
    }

    func queueDepth() async -> Int { await outbox.count() }
    func lastSyncedHLC() -> HLCCursor? { lastSynced }
    func lastInbound() -> Date? { lastInboundAt }

    // MARK: - Loop

    private func runLoop() async {
        // Ensure the custom zone exists; CloudKit returns an error if it doesn't.
        await ensureZoneExists()

        while !Task.isCancelled {
            do {
                try await pushOutboxBatch()
                try await pullInboundBatch()
                consecutiveFailures = 0
            } catch {
                consecutiveFailures += 1
            }
            let delay = backoffDelay()
            try? await Task.sleep(nanoseconds: delay)
        }
    }

    private func ensureZoneExists() async {
        let zone = CKRecordZone(zoneID: zoneID)
        _ = try? await database.save(zone)
    }

    private func pushOutboxBatch() async throws {
        let batch = await outbox.all()
        guard !batch.isEmpty else { return }
        let records = batch.map { $0.toRecord(zoneID: zoneID) }
        let (results, _) = try await database.modifyRecords(
            saving: records,
            deleting: [],
            savePolicy: .ifServerRecordUnchanged,
            atomically: false
        )
        let succeeded = results.compactMap { (id, result) -> String? in
            if case .success = result { return id.recordName }
            return nil
        }
        if !succeeded.isEmpty {
            let succeededSet = Set(succeeded)
            try await outbox.remove(ids: succeededSet)
        }
    }

    private func pullInboundBatch() async throws {
        // Query records newer than the cursor. CloudKit can't `WHERE > tuple`,
        // so we filter by physicalMS and reconcile logical ties client-side.
        let predicate: NSPredicate
        if let cursor = lastSynced {
            predicate = NSPredicate(
                format: "%K > %@",
                ActantCKSchema.Field.hlcPhysicalMS,
                NSNumber(value: cursor.physicalMS)
            )
        } else {
            predicate = NSPredicate(value: true)
        }
        let query = CKQuery(recordType: ActantCKSchema.recordType, predicate: predicate)
        query.sortDescriptors = [
            NSSortDescriptor(key: ActantCKSchema.Field.hlcPhysicalMS, ascending: true),
            NSSortDescriptor(key: ActantCKSchema.Field.hlcLogical,    ascending: true),
        ]
        let (matchResults, _) = try await database.records(
            matching: query,
            inZoneWith: zoneID,
            desiredKeys: nil,
            resultsLimit: 200
        )
        var rows: [EventRow] = []
        for (_, result) in matchResults {
            if case .success(let record) = result,
               let row = EventRow.fromRecord(record) {
                rows.append(row)
            }
        }
        guard !rows.isEmpty else { return }
        let report = try await actant.ingest(rows)
        _ = report
        lastInboundAt = Date()
        if let maxHLC = rows.map(\.hlc).max() {
            lastSynced = maxHLC
        }
    }

    private func backoffDelay() -> UInt64 {
        // Healthy: drain every 5s. On failure: exponential up to 60s.
        let base: UInt64 = 5_000_000_000
        if consecutiveFailures == 0 { return base }
        let capped = min(consecutiveFailures, 4)
        return base * UInt64(1 << capped)
    }
}

#endif // canImport(CloudKit) && !os(Linux)
