import Foundation
import ActantTestSupport
import Testing
@testable import ActantDB

/// Cross-mode smoke tests for the `Actant` unified facade. Remote-mode
/// coverage runs everywhere via MockURLProtocol; spawned + embedded are
/// gated and skipped when their backing dependencies aren't available.
@Suite("Actant facade")
struct ActantModeTests {

    // MARK: - Remote mode

    @Test("Actant.remote(...) dispatches via ActantClient and returns a CommandOutcome")
    func remoteDispatch() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/command")
            #expect(request.httpMethod == "POST")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["command_type"] as? String == "create_session")
            #expect(body["workspace_id"] as? String == "ws_default")
            #expect(body["actor_id"]     as? String == "act_user")
            let resp = #"{"command_id":"cmd_1","event_id":"evt_1","result":{"session_id":"sess_1"}}"#
            return (200, ["content-type": "application/json"], Data(resp.utf8))
        }) {
            let actant = try await Self.makeRemoteActant()
            let outcome = try await actant.dispatch(
                command: "create_session",
                input: .object(["title": .string("test")]),
                idempotencyKey: nil
            )
            #expect(outcome.commandID == "cmd_1")
            #expect(outcome.eventID == "evt_1")
            #expect(outcome.result["session_id"]?.stringValue == "sess_1")
        }
    }

    @Test("Actant.remote(...) eventsSince adapts SyncSinceResponse into EventRow list")
    func remoteEventsSince() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/sync/since")
            let resp = """
            {"events":[{
              "id":"evt_99","event_type":"user_message_received","actor_id":"act_user",
              "payload_hash":"h","payload_inline":"{\\"x\\":1}",
              "created_at":"2026-05-19T00:00:00Z"
            }],"next_since":null}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let actant = try await Self.makeRemoteActant()
            let rows = try await actant.eventsSince(nil, limit: 100)
            #expect(rows.count == 1)
            #expect(rows[0].id == "evt_99")
            #expect(rows[0].eventType == "user_message_received")
            #expect(rows[0].actorID == "act_user")
        }
    }

    @Test("Actant.remote(...) ingest is unsupported and surfaces a clear error")
    func remoteIngestUnsupported() async throws {
        let actant = try await Self.makeRemoteActant()
        do {
            _ = try await actant.ingest([])
            Issue.record("expected throw")
        } catch let ActantError.transport(msg) {
            #expect(msg.contains("embedded mode"))
        }
    }

    // MARK: - Spawned mode (host-only)

    #if !os(iOS)
    @Test("Actant.spawned(...) drives a SpawnedSupervisor through ensureRunning")
    func spawnedFacade() async throws {
        // Use a stub supervisor — the real ActantDBSupervisor lives in the
        // ActantAgent target and would require the actantdb binary.
        let supervisor = StubSupervisor(baseURL: URL(string: "http://127.0.0.1:55555")!)
        // We can't actually round-trip a command without a real server, but
        // we can assert the construction path works and that the supervisor
        // was driven exactly once.
        let actant = try await Actant.spawned(
            supervisor,
            dbPath: URL(fileURLWithPath: "/tmp/actantdb-modetest.db"),
            workspaceID: "ws_default",
            actorID: "act_user"
        )
        if case .spawned = await actant.mode { /* ok */ } else {
            Issue.record("expected .spawned mode")
        }
        let calls = await supervisor.callCount()
        #expect(calls == 1)
    }
    #endif

    // MARK: - Embedded mode

    #if canImport(ActantFFI)
    @Test("Actant.embedded(...) constructs through ActantFFIBridge when the FFI is linked")
    func embeddedConstructs() async throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("actantdb-modetest-\(UUID().uuidString)")
        let actant = try await Actant.embedded(
            storeDir: tmp,
            workspaceID: "ws_default",
            actorID: "act_user"
        )
        #expect(await actant.workspaceID == "ws_default")
    }
    #else
    @Test("Actant.embedded(...) fails fast when ActantFFI binary target is missing")
    func embeddedFailsWithoutFFI() async throws {
        let tmp = FileManager.default.temporaryDirectory
            .appendingPathComponent("actantdb-modetest-\(UUID().uuidString)")
        do {
            _ = try await Actant.embedded(
                storeDir: tmp,
                workspaceID: "ws_default",
                actorID: "act_user"
            )
            Issue.record("expected throw — ActantFFI is not linked in this build")
        } catch let ActantError.transport(msg) {
            #expect(msg.contains("ActantFFI"))
        }
    }
    #endif

    // MARK: - Helpers

    private static func makeRemoteActant() async throws -> Actant {
        // Hand-construct the client with the mock URLSession; we can't go
        // through `Actant.remote(_:...)` because that uses .shared internally.
        // Build the Actant via a small reflection-free helper that mirrors
        // what `remote(...)` does.
        let url = URL(string: "http://127.0.0.1:4555")!
        return try await Actant._testingRemote(
            baseURL: url,
            workspaceID: "ws_default",
            actorID: "act_user",
            urlSession: MockURLProtocol.makeSession()
        )
    }
}

// MARK: - Test-only Actant constructor that accepts a URLSession

extension Actant {
    /// Internal test constructor that lets us point the underlying
    /// `ActantClient` at a MockURLProtocol-backed `URLSession`. Production
    /// `remote(...)` uses `.shared`.
    static func _testingRemote(
        baseURL: URL,
        workspaceID: String,
        actorID: String,
        urlSession: URLSession
    ) async throws -> Actant {
        let client = ActantClient(baseURL: baseURL, urlSession: urlSession)
        return try await ActantTestingShim.make(
            client: client,
            workspaceID: workspaceID,
            actorID: actorID
        )
    }
}

// MARK: - Stub supervisor for the spawned-mode test

#if !os(iOS)
final class StubSupervisor: SpawnedSupervisor, @unchecked Sendable {
    private let baseURL: URL
    private let lock = NSLock()
    private var calls = 0

    init(baseURL: URL) { self.baseURL = baseURL }

    func ensureRunning(dbPath: URL) async throws -> URL {
        lock.withLock { calls += 1 }
        return baseURL
    }

    func callCount() -> Int {
        lock.withLock { calls }
    }
}
#endif
