import Foundation
import Testing
@testable import ActantDB

#if canImport(CloudKit) && !os(Linux)
import CloudKit

@Suite("CKRecord ↔ EventRow mapping")
struct CKRecordMappingTests {

    @Test("EventRow round-trips through CKRecord with every field preserved")
    func roundTrip() {
        let zone = CKRecordZone.ID(zoneName: "ActantDBLedger", ownerName: CKCurrentUserDefaultName)
        let original = EventRow(
            id: "evt_round_trip_1",
            workspaceID: "ws_default",
            sessionID: "sess_42",
            deviceID: "device-abc",
            actorID: "act_user",
            eventType: "user_message_received",
            payloadJSON: Data(#"{"text":"hi"}"#.utf8),
            payloadHash: "sha256-abc",
            prevChainHash: "sha256-prev",
            hlc: HLCCursor(physicalMS: 1_700_000_000_123, logical: 7),
            createdAt: "2026-05-19T00:00:00Z"
        )
        let record = original.toRecord(zoneID: zone)
        guard let decoded = EventRow.fromRecord(record) else {
            Issue.record("fromRecord returned nil")
            return
        }
        #expect(decoded.id == original.id)
        #expect(decoded.workspaceID == original.workspaceID)
        #expect(decoded.sessionID == original.sessionID)
        #expect(decoded.deviceID == original.deviceID)
        #expect(decoded.actorID == original.actorID)
        #expect(decoded.eventType == original.eventType)
        #expect(decoded.payloadJSON == original.payloadJSON)
        #expect(decoded.payloadHash == original.payloadHash)
        #expect(decoded.prevChainHash == original.prevChainHash)
        #expect(decoded.hlc == original.hlc)
        #expect(decoded.createdAt == original.createdAt)
    }

    @Test("EventRow with nil session_id and prev_chain_hash still round-trips")
    func roundTripWithNullables() {
        let zone = CKRecordZone.ID(zoneName: "ActantDBLedger", ownerName: CKCurrentUserDefaultName)
        let original = EventRow(
            id: "evt_round_trip_2",
            workspaceID: "ws_default",
            sessionID: nil,
            deviceID: "device-xyz",
            actorID: "act_agent",
            eventType: "agent_run_started",
            payloadJSON: Data(),
            payloadHash: "sha256-empty",
            prevChainHash: nil,
            hlc: HLCCursor(physicalMS: 0, logical: 0),
            createdAt: "2026-05-19T00:00:01Z"
        )
        let record = original.toRecord(zoneID: zone)
        let decoded = EventRow.fromRecord(record)
        #expect(decoded?.id == "evt_round_trip_2")
        #expect(decoded?.sessionID == nil)
        #expect(decoded?.prevChainHash == nil)
        #expect(decoded?.hlc == HLCCursor(physicalMS: 0, logical: 0))
    }

    @Test("fromRecord returns nil for records of the wrong type")
    func wrongTypeReturnsNil() {
        let zone = CKRecordZone.ID(zoneName: "ActantDBLedger", ownerName: CKCurrentUserDefaultName)
        let bogus = CKRecord(
            recordType: "NotActantEvent",
            recordID: CKRecord.ID(recordName: "bogus", zoneID: zone)
        )
        #expect(EventRow.fromRecord(bogus) == nil)
    }
}

#endif // canImport(CloudKit) && !os(Linux)
