import Foundation
import ActantTestSupport
import Testing
@testable import ActantAgent

@Suite("Auditor")
struct AuditorTests {

    struct AuditRecord: Codable, Sendable, Equatable {
        let kind: String
        let detail: String
        let n: Int
    }

    @Test("log then last round-trips a Codable record through the sentinel")
    func roundtrips() async throws {
        // Capture the text the SDK posted on `log`, then play it back as
        // a single event when `last` queries the session.
        actor Captured {
            var loggedText: String?
            func set(_ t: String) { loggedText = t }
            func get() -> String? { loggedText }
        }
        let captured = Captured()

        // ---- 1) log ----
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/command")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["command_type"] as? String == "append_agent_message")
            let input = body["input"] as! [String: Any]
            let text = input["text"] as! String
            #expect(text.contains("\"swoosh.audit\""))
            #expect(text.contains("\"kind\":\"effect_observed\""))
            // Capture for the next request.
            Task { await captured.set(text) }
            let resp = #"{"command_id":"cmd_1","event_id":"evt_1","result":{"message_id":"m1"}}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let auditor = Auditor<AuditRecord>(
                backend: backend,
                sessionID: "sess_audit",
                sentinelKey: "swoosh.audit"
            )
            try await auditor.log(AuditRecord(
                kind: "effect_observed", detail: "shell.run completed", n: 7
            ))
        }

        // Synchronize: the Task above may not have completed yet. Wait
        // briefly until `captured.get()` is non-nil.
        var loggedText: String?
        for _ in 0..<50 {
            if let t = await captured.get() { loggedText = t; break }
            try await Task.sleep(nanoseconds: 5_000_000) // 5ms
        }
        guard let loggedText else {
            Issue.record("log handler never captured the request body")
            return
        }

        // ---- 2) last ----
        // Build an events response whose `payload_inline` carries the
        // exact text the SDK sent, wrapped in `{"message_id":..,"text":..}`.
        // We need to embed `loggedText` as a JSON string literal inside the
        // outer payload_inline string. JSONSerialization gives us the right
        // escaping for free.
        let payloadObj: [String: Any] = ["message_id": "m1", "text": loggedText]
        let payloadData = try JSONSerialization.data(withJSONObject: payloadObj, options: [])
        let payloadStr = String(data: payloadData, encoding: .utf8)!
        // Now embed payloadStr as a JSON-string value inside the wrapping events JSON.
        let wrappingObj: [String: Any] = [
            "events": [[
                "id": "evt_1", "workspace_id": "w", "actor_id": "a", "session_id": "sess_audit",
                "parent_event_id": NSNull(), "event_type": "agent_message",
                "causality_kind": "observation", "sensitivity": "low",
                "authority_scope_id": NSNull(), "payload_inline": payloadStr, "payload_ref": NSNull(),
                "payload_hash": "h", "event_hash": "eh", "created_at": "2026-05-18T00:00:00Z",
                "model_call_id": NSNull(), "tool_call_id": NSNull(), "workflow_run_id": NSNull(),
                "memory_id": NSNull(), "artifact_id": NSNull(), "command_id": NSNull(),
                "effect_id": NSNull(),
            ]],
        ]
        let wrappingData = try JSONSerialization.data(withJSONObject: wrappingObj, options: [])

        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/events")
            return (200, [:], wrappingData)
        }) {
            let backend = makeBackend()
            let auditor = Auditor<AuditRecord>(
                backend: backend,
                sessionID: "sess_audit",
                sentinelKey: "swoosh.audit"
            )
            let got = try await auditor.last()
            #expect(got == AuditRecord(
                kind: "effect_observed", detail: "shell.run completed", n: 7
            ))
        }
    }

    @Test("last returns nil when no events carry the sentinel")
    func lastReturnsNilWhenAbsent() async throws {
        try await MockURLProtocol.with({ _ in
            let resp = #"{"events":[]}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let auditor = Auditor<AuditRecord>(
                backend: backend,
                sessionID: "sess_audit",
                sentinelKey: "swoosh.audit"
            )
            let got = try await auditor.last()
            #expect(got == nil)
        }
    }
}
