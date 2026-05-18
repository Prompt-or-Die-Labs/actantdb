import Foundation
import Testing
@testable import ActantDB

@Suite("Commands + queries")
struct CommandTests {

    private func makeClient(token: String? = nil) -> ActantClient {
        ActantClient(
            baseURL: URL(string: "http://127.0.0.1:4555")!,
            token: token,
            urlSession: MockURLProtocol.makeSession()
        )
    }

    @Test("createSession POSTs /v1/command with type create_session and pulls session_id from result")
    func createSession() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/command")
            #expect(request.httpMethod == "POST")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["command_type"] as? String == "create_session")
            #expect(body["workspace_id"] as? String == "ws_default")
            #expect(body["actor_id"] as? String == "act_user")
            let input = body["input"] as! [String: Any]
            #expect(input["title"] as? String == "test")
            let resp = #"{"command_id":"cmd_1","event_id":"evt_1","result":{"session_id":"sess_1"}}"#
            return (200, ["content-type": "application/json"], Data(resp.utf8))
        }) {
            let id = try await makeClient().createSession(
                workspaceID: "ws_default", actorID: "act_user", title: "test"
            )
            #expect(id == "sess_1")
        }
    }

    @Test("requestToolCall serializes nested JSONValue arguments")
    func requestToolCall() async throws {
        try await MockURLProtocol.with({ request in
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            let input = body["input"] as! [String: Any]
            #expect(input["tool_name"] as? String == "shell.run")
            let args = input["arguments"] as! [String: Any]
            #expect(args["cmd"] as? String == "ls")
            let resp = #"{"command_id":"cmd_2","event_id":"evt_2","result":{}}"#
            return (200, [:], Data(resp.utf8))
        }) {
            _ = try await makeClient().requestToolCall(
                workspaceID: "ws", actorID: "a", sessionID: "s",
                toolName: "shell.run", arguments: ["cmd": "ls"]
            )
        }
    }

    @Test("events() sends session_id query and decodes AgentEvent array")
    func eventsQuery() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/events")
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!.queryItems!
            #expect(qs.first(where: { $0.name == "session_id" })?.value == "sess_42")
            let resp = """
            {"events":[{
              "id":"evt_1","workspace_id":"w","actor_id":"a","session_id":"sess_42",
              "parent_event_id":null,"event_type":"user_message_received",
              "causality_kind":"observation","sensitivity":"low",
              "authority_scope_id":null,"payload_inline":null,"payload_ref":null,
              "payload_hash":"h","event_hash":"eh","created_at":"2026-05-18T00:00:00Z",
              "model_call_id":null,"tool_call_id":null,"workflow_run_id":null,
              "memory_id":null,"artifact_id":null,"command_id":null,"effect_id":null
            }]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let events = try await makeClient().events(sessionID: "sess_42")
            #expect(events.count == 1)
            #expect(events[0].eventType == "user_message_received")
        }
    }

    @Test("approvals() sends workspace_id and decodes PendingApproval rows")
    func approvalsQuery() async throws {
        try await MockURLProtocol.with({ request in
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!.queryItems!
            #expect(qs.first(where: { $0.name == "workspace_id" })?.value == "ws_default")
            let resp = #"{"approvals":[{"id":"appr_1","tool_call_id":"tc_1","requested_by":"act_agent","risk_level":"high","summary":"rm -rf","status":"pending"}]}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let pending = try await makeClient().approvals(workspaceID: "ws_default")
            #expect(pending.first?.riskLevel == "high")
        }
    }

    @Test("HTTP 202 (approval_required) surfaces as ActantError.http with kind=approval_required")
    func approvalRequired202() async throws {
        try await MockURLProtocol.with({ _ in
            let body = #"{"error":"approval_required","message":"requires approval"}"#
            return (202, [:], Data(body.utf8))
        }) {
            do {
                _ = try await makeClient().createSession(workspaceID: "w", actorID: "a")
                Issue.record("expected throw")
            } catch let ActantError.http(status, kind, _, _) {
                #expect(status == 202)
                #expect(kind == "approval_required")
            }
        }
    }

    @Test("HTTP 429 rate_limited preserves retry semantics in body")
    func rateLimited429() async throws {
        try await MockURLProtocol.with({ _ in
            let body = #"{"error":"rate_limited","retry_after_seconds":5}"#
            return (429, ["retry-after": "5"], Data(body.utf8))
        }) {
            do {
                _ = try await makeClient().createSession(workspaceID: "w", actorID: "a")
                Issue.record("expected throw")
            } catch let ActantError.http(status, kind, _, _) {
                #expect(status == 429)
                #expect(kind == "rate_limited")
            }
        }
    }

    @Test("replayRun POSTs actor_id, checkpoint_id, mode and decodes ReplayDiff")
    func replayRun() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/replay/run")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["mode"] as? String == "memory")
            let resp = #"{"a":"r1","b":"r2","entries":[]}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let diff = try await makeClient().replayRun(
                actorID: "act_user", checkpointID: "chk_1", mode: .memory
            )
            #expect(diff.a == "r1")
            #expect(diff.b == "r2")
        }
    }

    @Test("syncSince forwards since_event_id + limit; decodes SyncSinceResponse")
    func syncSince() async throws {
        try await MockURLProtocol.with({ request in
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["since_event_id"] as? String == "evt_42")
            #expect(body["limit"] as? Int == 500)
            let resp = #"{"events":[],"next_since":null}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let r = try await makeClient().syncSince(workspaceID: "w", sinceEventID: "evt_42", limit: 500)
            #expect(r.events.isEmpty)
            #expect(r.nextSince == nil)
        }
    }
}

// MARK: - URLRequest body helper (URLProtocol strips httpBody — read from stream)

extension URLRequest {
    func bodyData() -> Data {
        if let body = httpBody { return body }
        guard let stream = httpBodyStream else { return Data() }
        var data = Data()
        stream.open()
        defer { stream.close() }
        var buf = [UInt8](repeating: 0, count: 4096)
        while stream.hasBytesAvailable {
            let n = stream.read(&buf, maxLength: buf.count)
            if n <= 0 { break }
            data.append(buf, count: n)
        }
        return data
    }
}
