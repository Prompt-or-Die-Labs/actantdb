import Foundation
import Testing
@testable import ActantDB

@Suite("CloudKit outbox persistence")
struct CloudKitOutboxStoreTests {
    @Test("queued events reload from disk on restart")
    func resumesQueuedEventsAfterRestart() async throws {
        let fileURL = Self.tempFileURL()
        defer { try? FileManager.default.removeItem(at: fileURL.deletingLastPathComponent()) }

        let store = try CloudKitOutboxStore(fileURL: fileURL)
        try await store.append([Self.event(id: "evt_1"), Self.event(id: "evt_2")])

        let reopened = try CloudKitOutboxStore(fileURL: fileURL)
        let events = await reopened.all()

        #expect(events.map(\.id) == ["evt_1", "evt_2"])
    }

    @Test("successful uploads are removed and failed events remain retryable")
    func removesOnlySucceededEvents() async throws {
        let fileURL = Self.tempFileURL()
        defer { try? FileManager.default.removeItem(at: fileURL.deletingLastPathComponent()) }

        let store = try CloudKitOutboxStore(fileURL: fileURL)
        try await store.append([
            Self.event(id: "evt_uploaded"),
            Self.event(id: "evt_retry"),
        ])
        try await store.remove(ids: Set(["evt_uploaded"]))

        let reopened = try CloudKitOutboxStore(fileURL: fileURL)
        let events = await reopened.all()

        #expect(events.map(\.id) == ["evt_retry"])
        #expect(await reopened.count() == 1)
    }

    @Test("duplicate enqueue preserves one durable event")
    func duplicateEnqueueIsIdempotent() async throws {
        let fileURL = Self.tempFileURL()
        defer { try? FileManager.default.removeItem(at: fileURL.deletingLastPathComponent()) }

        let event = Self.event(id: "evt_dup")
        let store = try CloudKitOutboxStore(fileURL: fileURL)
        try await store.append([event, event])

        let reopened = try CloudKitOutboxStore(fileURL: fileURL)

        #expect(await reopened.count() == 1)
    }

    private static func tempFileURL() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("actantdb-swift-outbox-\(UUID().uuidString)", isDirectory: true)
            .appendingPathComponent("outbox.json")
    }

    private static func event(id: String) -> EventRow {
        EventRow(
            id: id,
            workspaceID: "ws_default",
            sessionID: "sess_1",
            deviceID: "device_1",
            actorID: "act_agent",
            eventType: "tool_call_completed",
            payloadJSON: Data(#"{"ok":true}"#.utf8),
            payloadHash: "sha256-\(id)",
            prevChainHash: nil,
            hlc: HLCCursor(physicalMS: 1_700_000_000_000, logical: 1),
            createdAt: "2026-05-22T00:00:00Z"
        )
    }
}
