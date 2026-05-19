#if canImport(CloudKit) && !os(Linux)
import Foundation
import CloudKit

/// CKRecord schema for the replication ledger. One `EventRow` maps to one
/// `CKRecord(recordType: "ActantEvent")` per the schema in
/// `docs/SYNC_DESIGN.md`.
enum ActantCKSchema {
    static let recordType = "ActantEvent"

    enum Field {
        static let id              = "id"
        static let workspaceID     = "workspace_id"
        static let sessionID       = "session_id"
        static let deviceID        = "device_id"
        static let actorID         = "actor_id"
        static let eventType       = "event_type"
        static let payloadJSON     = "payload_json"
        static let payloadHash     = "payload_hash"
        static let prevChainHash   = "prev_chain_hash"
        static let hlcPhysicalMS   = "hlc_physical_ms"
        static let hlcLogical      = "hlc_logical"
        static let createdAt       = "created_at"
    }
}

extension EventRow {
    /// Encode this row as a CKRecord. The record name is the content-derived
    /// `id` so concurrent writes from two devices producing the same logical
    /// event collapse into one CloudKit record (last writer's bytes win, but
    /// since the id is content-derived the bytes are identical).
    func toRecord(zoneID: CKRecordZone.ID) -> CKRecord {
        let recordID = CKRecord.ID(recordName: id, zoneID: zoneID)
        let record = CKRecord(recordType: ActantCKSchema.recordType, recordID: recordID)
        record[ActantCKSchema.Field.id]              = id as CKRecordValue
        record[ActantCKSchema.Field.workspaceID]     = workspaceID as CKRecordValue
        if let sessionID {
            record[ActantCKSchema.Field.sessionID]   = sessionID as CKRecordValue
        }
        record[ActantCKSchema.Field.deviceID]        = deviceID as CKRecordValue
        record[ActantCKSchema.Field.actorID]         = actorID as CKRecordValue
        record[ActantCKSchema.Field.eventType]       = eventType as CKRecordValue
        record[ActantCKSchema.Field.payloadJSON]     = payloadJSON as CKRecordValue
        record[ActantCKSchema.Field.payloadHash]     = payloadHash as CKRecordValue
        if let prevChainHash {
            record[ActantCKSchema.Field.prevChainHash] = prevChainHash as CKRecordValue
        }
        record[ActantCKSchema.Field.hlcPhysicalMS]   = NSNumber(value: hlc.physicalMS)
        record[ActantCKSchema.Field.hlcLogical]      = NSNumber(value: hlc.logical)
        record[ActantCKSchema.Field.createdAt]       = createdAt as CKRecordValue
        return record
    }

    /// Decode a CKRecord back into an EventRow. Returns nil for records that
    /// are missing required fields (corruption, schema mismatch) so the
    /// caller can quarantine them without crashing the drainer.
    static func fromRecord(_ rec: CKRecord) -> EventRow? {
        guard rec.recordType == ActantCKSchema.recordType,
              let id          = rec[ActantCKSchema.Field.id]          as? String,
              let workspaceID = rec[ActantCKSchema.Field.workspaceID] as? String,
              let deviceID    = rec[ActantCKSchema.Field.deviceID]    as? String,
              let actorID     = rec[ActantCKSchema.Field.actorID]     as? String,
              let eventType   = rec[ActantCKSchema.Field.eventType]   as? String,
              let payloadJSON = rec[ActantCKSchema.Field.payloadJSON] as? Data,
              let payloadHash = rec[ActantCKSchema.Field.payloadHash] as? String,
              let physicalNum = rec[ActantCKSchema.Field.hlcPhysicalMS] as? NSNumber,
              let logicalNum  = rec[ActantCKSchema.Field.hlcLogical]    as? NSNumber,
              let createdAt   = rec[ActantCKSchema.Field.createdAt]   as? String
        else {
            return nil
        }
        let sessionID     = rec[ActantCKSchema.Field.sessionID]     as? String
        let prevChainHash = rec[ActantCKSchema.Field.prevChainHash] as? String
        return EventRow(
            id: id,
            workspaceID: workspaceID,
            sessionID: sessionID,
            deviceID: deviceID,
            actorID: actorID,
            eventType: eventType,
            payloadJSON: payloadJSON,
            payloadHash: payloadHash,
            prevChainHash: prevChainHash,
            hlc: HLCCursor(
                physicalMS: physicalNum.int64Value,
                logical: logicalNum.uint32Value
            ),
            createdAt: createdAt
        )
    }
}

#endif // canImport(CloudKit) && !os(Linux)
