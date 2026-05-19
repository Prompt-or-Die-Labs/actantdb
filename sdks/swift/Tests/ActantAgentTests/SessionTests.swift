import Foundation
import ActantTestSupport
import Testing
@testable import ActantAgent

@Suite("Session")
struct SessionTests {

    /// Consumer's own message type. The SDK has no knowledge of this.
    struct ChatMessage: Equatable, Sendable {
        let role: SessionRole
        let text: String
        let createdAt: Date
    }

    private static func makeSession(backend: AgentBackend, sessionID: String = "sess_42")
        -> Session<ChatMessage>
    {
        Session<ChatMessage>(
            backend: backend,
            sessionID: sessionID,
            encode: { msg in (msg.role, msg.text) },
            decode: { role, text, date in ChatMessage(role: role, text: text, createdAt: date) }
        )
    }

    @Test("appendMessage(.user) dispatches append_user_message with session_id+text")
    func appendUserDispatches() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/command")
            #expect(request.httpMethod == "POST")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["command_type"] as? String == "append_user_message")
            let input = body["input"] as! [String: Any]
            #expect(input["session_id"] as? String == "sess_42")
            #expect(input["text"] as? String == "hello")
            let resp = #"{"command_id":"cmd_1","event_id":"evt_1","result":{"message_id":"m1"}}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let session = Self.makeSession(backend: backend)
            try await session.appendMessage(
                ChatMessage(role: .user, text: "hello", createdAt: Date())
            )
        }
    }

    @Test("appendMessage(.assistant) dispatches append_agent_message")
    func appendAssistantDispatches() async throws {
        try await MockURLProtocol.with({ request in
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["command_type"] as? String == "append_agent_message")
            let input = body["input"] as! [String: Any]
            #expect(input["text"] as? String == "ack")
            let resp = #"{"command_id":"cmd_2","event_id":"evt_2","result":{"message_id":"m2"}}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let session = Self.makeSession(backend: backend)
            try await session.appendMessage(
                ChatMessage(role: .assistant, text: "ack", createdAt: Date())
            )
        }
    }

    @Test("loadTranscript parses user+agent events with the supplied decode closure")
    func loadTranscriptRoundtrips() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/events")
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!.queryItems!
            #expect(qs.first(where: { $0.name == "session_id" })?.value == "sess_42")
            // Two events: user_message_received then agent_message.
            // payload_inline is a JSON *string* (escaped) per the server's
            // storage row schema.
            let userPayload = #"{\"message_id\":\"m1\",\"text\":\"hi\"}"#
            let agentPayload = #"{\"message_id\":\"m2\",\"text\":\"hello back\"}"#
            let resp = """
            {"events":[
              {"id":"evt_1","workspace_id":"w","actor_id":"a","session_id":"sess_42",
               "parent_event_id":null,"event_type":"user_message_received",
               "causality_kind":"observation","sensitivity":"low",
               "authority_scope_id":null,"payload_inline":"\(userPayload)","payload_ref":null,
               "payload_hash":"h1","event_hash":"eh1","created_at":"2026-05-18T00:00:00Z",
               "model_call_id":null,"tool_call_id":null,"workflow_run_id":null,
               "memory_id":null,"artifact_id":null,"command_id":null,"effect_id":null},
              {"id":"evt_2","workspace_id":"w","actor_id":"a","session_id":"sess_42",
               "parent_event_id":null,"event_type":"agent_message",
               "causality_kind":"observation","sensitivity":"low",
               "authority_scope_id":null,"payload_inline":"\(agentPayload)","payload_ref":null,
               "payload_hash":"h2","event_hash":"eh2","created_at":"2026-05-18T00:00:01Z",
               "model_call_id":null,"tool_call_id":null,"workflow_run_id":null,
               "memory_id":null,"artifact_id":null,"command_id":null,"effect_id":null}
            ]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let session = Self.makeSession(backend: backend)
            let messages = try await session.loadTranscript()
            #expect(messages.count == 2)
            #expect(messages[0].role == .user)
            #expect(messages[0].text == "hi")
            #expect(messages[1].role == .assistant)
            #expect(messages[1].text == "hello back")
        }
    }
}
