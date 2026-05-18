import Foundation
import Testing
@testable import ActantDB

@Suite("Codable round-trips")
struct CodableTests {

    // MARK: - JSONValue

    @Test("JSONValue decodes the full JSON shape (null/bool/int/double/string/array/object)")
    func jsonValueDecodes() throws {
        let src = #"{"a":1,"b":"x","c":true,"d":null,"e":[1,2.5],"f":{"g":false}}"#.data(using: .utf8)!
        let v = try JSONDecoder().decode(JSONValue.self, from: src)
        guard case .object(let o) = v else { Issue.record("expected object"); return }
        #expect(o["a"]?.intValue == 1)
        #expect(o["b"]?.stringValue == "x")
        #expect(o["c"]?.boolValue == true)
        if case .null = o["d"]! { } else { Issue.record("expected null") }
        if case .array(let a) = o["e"]! {
            #expect(a[0].intValue == 1)
            #expect(a[1] == .double(2.5))
        } else { Issue.record("expected array") }
        #expect(o["f"]?["g"]?.boolValue == false)
    }

    @Test("JSONValue round-trips through encode→decode")
    func jsonValueRoundTrip() throws {
        let v: JSONValue = ["k": ["nested": [1, 2, 3]], "n": .null, "b": true]
        let data = try JSONEncoder().encode(v)
        let decoded = try JSONDecoder().decode(JSONValue.self, from: data)
        #expect(decoded == v)
    }

    // MARK: - Enums (serde renames)

    @Test("Sensitivity wire format is lowercase")
    func sensitivityRename() throws {
        #expect(try JSONDecoder().decode(Sensitivity.self, from: Data(#""secret""#.utf8)) == .secret)
        let enc = String(data: try JSONEncoder().encode(Sensitivity.high), encoding: .utf8)
        #expect(enc == #""high""#)
    }

    @Test("EventKind wire format is snake_case")
    func eventKindRename() throws {
        let d = try JSONDecoder().decode(EventKind.self, from: Data(#""tool_call_completed""#.utf8))
        #expect(d == .toolCallCompleted)
        let enc = String(data: try JSONEncoder().encode(EventKind.agentRunFinished), encoding: .utf8)
        #expect(enc == #""agent_run_finished""#)
    }

    @Test("CausalityKind decodes lowercase")
    func causalityRename() throws {
        #expect(try JSONDecoder().decode(CausalityKind.self, from: Data(#""effect""#.utf8)) == .effect)
    }

    // MARK: - PolicyVerdict (internally-tagged union)

    @Test("PolicyVerdict.allow round-trips through internal tag")
    func policyVerdictAllow() throws {
        let v: PolicyVerdict = .allow(reason: "ok", policySnapshot: "abc")
        let data = try JSONEncoder.actant.encode(v)
        let s = String(data: data, encoding: .utf8)!
        #expect(s.contains(#""decision":"allow""#))
        #expect(s.contains(#""reason":"ok""#))
        #expect(s.contains(#""policy_snapshot":"abc""#))
        let back = try JSONDecoder().decode(PolicyVerdict.self, from: data)
        #expect(back == v)
    }

    @Test("PolicyVerdict.constrain carries hint + constrained_input")
    func policyVerdictConstrain() throws {
        let v: PolicyVerdict = .constrain(
            reason: "narrow scope",
            policySnapshot: "snap1",
            constrainedInput: ["cmd": "rm -rf build"],
            hint: "drop dist"
        )
        let data = try JSONEncoder.actant.encode(v)
        let back = try JSONDecoder().decode(PolicyVerdict.self, from: data)
        #expect(back == v)
        let raw = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        #expect(raw["decision"] as? String == "constrain")
        #expect(raw["hint"] as? String == "drop dist")
        #expect((raw["constrained_input"] as? [String: Any])?["cmd"] as? String == "rm -rf build")
    }

    @Test("PolicyVerdict.requireApproval allows missing hint + constrained_input")
    func policyVerdictRequireApproval() throws {
        let wire = #"{"decision":"require_approval","reason":"risky","policy_snapshot":"s1"}"#
        let v = try JSONDecoder().decode(PolicyVerdict.self, from: Data(wire.utf8))
        guard case let .requireApproval(_, _, hint, input) = v else {
            Issue.record("expected requireApproval"); return
        }
        #expect(hint == nil)
        #expect(input == nil)
    }

    @Test("PolicyVerdict rejects unknown decision string")
    func policyVerdictUnknown() throws {
        let wire = #"{"decision":"explode","reason":"x","policy_snapshot":"x"}"#
        #expect(throws: DecodingError.self) {
            try JSONDecoder().decode(PolicyVerdict.self, from: Data(wire.utf8))
        }
    }

    // MARK: - ApprovalDecisionV

    @Test("ApprovalDecisionV round-trips all three variants")
    func approvalDecisionRoundTrip() throws {
        let variants: [ApprovalDecisionV] = [
            .approve(approver: "u1", scope: "once"),
            .approveConstrained(approver: "u1", scope: "session",
                                acceptedInput: ["cmd": "rm -rf build"]),
            .deny(approver: "u2", reason: "no"),
        ]
        for v in variants {
            let data = try JSONEncoder.actant.encode(v)
            let back = try JSONDecoder().decode(ApprovalDecisionV.self, from: data)
            #expect(back == v)
        }
    }

    @Test("ApprovalDecisionV decision tag values match server")
    func approvalDecisionTags() throws {
        let raw1 = try JSONSerialization.jsonObject(
            with: try JSONEncoder.actant.encode(ApprovalDecisionV.approve(approver: "u", scope: "once"))
        ) as! [String: Any]
        #expect(raw1["decision"] as? String == "approve")

        let raw2 = try JSONSerialization.jsonObject(
            with: try JSONEncoder.actant.encode(ApprovalDecisionV.approveConstrained(
                approver: "u", scope: "once", acceptedInput: .null
            ))
        ) as! [String: Any]
        #expect(raw2["decision"] as? String == "approve_constrained")
    }

    // MARK: - AgentEvent + ActantEvent

    @Test("AgentEvent decodes the storage row shape with optional backrefs")
    func agentEventDecodes() throws {
        let wire = """
        {
          "id": "evt_01ABC",
          "workspace_id": "ws_default",
          "actor_id": "act_user",
          "session_id": "sess_1",
          "parent_event_id": null,
          "event_type": "tool_call_completed",
          "causality_kind": "effect",
          "sensitivity": "low",
          "authority_scope_id": null,
          "payload_inline": "{\\"status\\":\\"ok\\"}",
          "payload_ref": null,
          "payload_hash": "deadbeef",
          "event_hash": "cafef00d",
          "created_at": "2026-05-18T12:00:00Z",
          "model_call_id": null,
          "tool_call_id": "tc_1",
          "workflow_run_id": null,
          "memory_id": null,
          "artifact_id": null,
          "command_id": null,
          "effect_id": null
        }
        """.data(using: .utf8)!
        let e = try JSONDecoder().decode(AgentEvent.self, from: wire)
        #expect(e.id == "evt_01ABC")
        #expect(e.eventType == "tool_call_completed")
        #expect(e.causalityKind == .effect)
        #expect(e.toolCallID == "tc_1")
        let payload = try e.parsedPayload()
        #expect(payload?["status"]?.stringValue == "ok")
    }

    @Test("ActantEvent (contracts shape) decodes with typed kind + JSONValue payload")
    func actantEventDecodes() throws {
        let wire = """
        {
          "id": "evt_2",
          "kind": "model_call",
          "project": "demo",
          "run_id": "run_1",
          "payload": {"summary":"plan A"},
          "payload_hash": "h1",
          "chain_hash": "h2",
          "sensitivity": "medium",
          "created_at": "2026-05-18T12:00:00Z"
        }
        """.data(using: .utf8)!
        let e = try JSONDecoder().decode(ActantEvent.self, from: wire)
        #expect(e.kind == .modelCall)
        #expect(e.sensitivity == .medium)
        #expect(e.payload["summary"]?.stringValue == "plan A")
        #expect(e.parentEventID == nil)
    }
}
