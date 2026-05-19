#if canImport(CloudKit) && !os(Linux)
import Foundation
import CloudKit

/// Owns the `CKQuerySubscription` (or `CKRecordZoneSubscription`) registered
/// against the workspace's record zone, plus the APNs push registration that
/// wakes the device when a peer commits an event.
///
/// The subscription ID is deterministic so re-registering on every app
/// launch is idempotent — CloudKit returns "subscription already exists"
/// which we treat as success.
actor SubscriptionManager {
    private let database: CKDatabase
    private let containerID: String
    private var active: Bool = false

    private var subscriptionID: CKSubscription.ID {
        "actantdb.\(containerID).events"
    }

    init(database: CKDatabase, containerID: String) {
        self.database = database
        self.containerID = containerID
    }

    func registerIfNeeded() async throws {
        // A query subscription on the ActantEvent record type is the simplest
        // shape that gets us "wake on new event"; a record-zone subscription
        // would be more efficient at scale, swap in when a consumer asks.
        let predicate = NSPredicate(value: true)
        let subscription = CKQuerySubscription(
            recordType: ActantCKSchema.recordType,
            predicate: predicate,
            subscriptionID: subscriptionID,
            options: [.firesOnRecordCreation]
        )
        let info = CKSubscription.NotificationInfo()
        info.shouldSendContentAvailable = true   // silent push, app wakes to drain
        subscription.notificationInfo = info

        do {
            _ = try await database.save(subscription)
            active = true
        } catch let error as CKError where error.code == .serverRejectedRequest {
            // Most often "subscription already exists" — treat as success.
            active = true
        } catch {
            throw error
        }
    }

    func unregister() async throws {
        do {
            try await database.deleteSubscription(withID: subscriptionID)
        } catch let error as CKError where error.code == .unknownItem {
            // Already gone; treat as success.
        }
        active = false
    }

    func isActive() -> Bool { active }
}

#endif // canImport(CloudKit) && !os(Linux)
