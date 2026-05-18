import Foundation
import Testing
@testable import ActantAgent

@Suite("MemoryStore")
struct MemoryStoreTests {

    @Test("propose dispatches propose_memory with the right input fields")
    func proposeDispatches() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/command")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["command_type"] as? String == "propose_memory")
            let input = body["input"] as! [String: Any]
            #expect(input["text"]        as? String == "user prefers dark mode")
            #expect(input["category"]    as? String == "preference")
            #expect(input["sensitivity"] as? String == "low")
            // JSON-coerced doubles can come back as Int when whole — accept either.
            let conf = (input["confidence"] as? Double) ?? Double(input["confidence"] as? Int ?? 0)
            #expect(conf == 0.9)
            let resp = #"{"command_id":"cmd_1","event_id":"evt_1","result":{"candidate_id":"mc_1"}}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let store = MemoryStore(backend: backend)
            let id = try await store.propose(
                text: "user prefers dark mode",
                category: "preference",
                sensitivity: .low,
                confidence: 0.9,
                evidence: .object(["src": .string("settings")])
            )
            #expect(id == "mc_1")
        }
    }

    @Test("listApproved decodes status=approved rows and filters out non-approved cases")
    func listApprovedRoundTrip() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/memories")
            #expect(request.httpMethod == "GET")
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
                .queryItems ?? []
            #expect(qs.first(where: { $0.name == "workspace_id" })?.value == "ws_default")
            #expect(qs.first(where: { $0.name == "status" })?.value == "approved")
            let resp = """
            {"memories":[
              {"id":"mem_1","workspace_id":"ws_default","text":"prefers dark mode",
               "category":"preference","sensitivity":"low","confidence":0.9,
               "scope":null,"source_candidate_id":"mc_1","usage_count":3,
               "last_used_at":null,"expires_at":null,"revoked_at":null,
               "deleted_at":null,"created_at":"2026-05-18T00:00:00Z",
               "status":"approved"}
            ]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let store = MemoryStore(backend: backend)
            let approved = try await store.listApproved()
            #expect(approved.count == 1)
            #expect(approved[0].id == "mem_1")
            #expect(approved[0].text == "prefers dark mode")
            #expect(approved[0].sensitivity == .low)
            #expect(approved[0].usageCount == 3)
        }
    }

    @Test("listPending decodes status=pending candidates")
    func listPendingRoundTrip() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/memories")
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
                .queryItems ?? []
            #expect(qs.first(where: { $0.name == "status" })?.value == "pending")
            let resp = """
            {"memories":[
              {"id":"mc_1","workspace_id":"ws_default","text":"likes spicy food",
               "category":"food","sensitivity":"low","confidence":0.7,
               "scope":null,"source_candidate_id":null,"usage_count":null,
               "last_used_at":null,"expires_at":null,"revoked_at":null,
               "deleted_at":null,"created_at":"2026-05-18T00:00:00Z",
               "status":"pending"}
            ]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let store = MemoryStore(backend: backend)
            let pending = try await store.listPending()
            #expect(pending.count == 1)
            #expect(pending[0].id == "mc_1")
            #expect(pending[0].status == "pending")
            #expect(pending[0].confidence == 0.7)
        }
    }

    @Test("listConflicts decodes MemoryConflict rows")
    func listConflictsRoundTrip() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/memories/conflicts")
            let resp = """
            {"conflicts":[
              {"id":"conf_1","workspace_id":"ws_default",
               "memory_a_id":"mem_1","memory_b_id":"mem_2",
               "conflict_type":"contradiction","resolution_policy":null,
               "last_resolved_at":null,"detected_at":"2026-05-18T01:00:00Z"}
            ]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let store = MemoryStore(backend: backend)
            let conflicts = try await store.listConflicts()
            #expect(conflicts.count == 1)
            #expect(conflicts[0].memoryAID == "mem_1")
            #expect(conflicts[0].memoryBID == "mem_2")
            #expect(conflicts[0].conflictType == "contradiction")
        }
    }
}
