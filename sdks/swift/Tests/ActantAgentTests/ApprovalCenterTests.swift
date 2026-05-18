import Foundation
import Testing
@testable import ActantAgent

@Suite("ApprovalCenter")
struct ApprovalCenterTests {

    @Test("approve dispatches approve_tool_call with tool_call_id + scope")
    func approveDispatches() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/command")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["command_type"] as? String == "approve_tool_call")
            let input = body["input"] as! [String: Any]
            #expect(input["tool_call_id"] as? String == "tc_1")
            #expect(input["scope"]        as? String == "once")
            let resp = #"{"command_id":"cmd_1","event_id":"evt_1","result":{}}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let center = ApprovalCenter(backend: backend)
            try await center.approve(toolCallID: "tc_1", scope: "once")
        }
    }

    @Test("approveConstrained dispatches approve_tool_call with accepted_input")
    func approveConstrainedDispatches() async throws {
        try await MockURLProtocol.with({ request in
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["command_type"] as? String == "approve_tool_call")
            let input = body["input"] as! [String: Any]
            #expect(input["tool_call_id"] as? String == "tc_2")
            #expect(input["scope"]        as? String == "session")
            let accepted = input["accepted_input"] as! [String: Any]
            #expect(accepted["cmd"] as? String == "ls /tmp")
            let resp = #"{"command_id":"cmd_2","event_id":"evt_2","result":{}}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let backend = makeBackend()
            let center = ApprovalCenter(backend: backend)
            try await center.approveConstrained(
                toolCallID: "tc_2",
                acceptedInput: .object(["cmd": .string("ls /tmp")]),
                scope: "session"
            )
        }
    }
}
