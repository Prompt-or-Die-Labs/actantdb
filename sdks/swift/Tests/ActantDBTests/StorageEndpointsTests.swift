import Foundation
import ActantTestSupport
import Testing
@testable import ActantDB

@Suite("Storage endpoints (memories / permissions / setup-reports / scout-records)")
struct StorageEndpointsTests {

    private func makeClient() -> ActantClient {
        ActantClient(
            baseURL: URL(string: "http://127.0.0.1:4555")!,
            token: nil,
            urlSession: MockURLProtocol.makeSession()
        )
    }

    // MARK: - Memories

    @Test("memories(status:) GETs /v1/memories with workspace_id+status; MemoryRow union decodes correctly")
    func memoriesStatusAll() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/memories")
            #expect(request.httpMethod == "GET")
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
                .queryItems ?? []
            #expect(qs.first(where: { $0.name == "workspace_id" })?.value == "ws_default")
            #expect(qs.first(where: { $0.name == "status" })?.value == "all")
            let resp = """
            {"memories":[
              {"id":"mem_1","workspace_id":"ws_default","text":"a","category":"x",
               "sensitivity":"low","confidence":0.5,"scope":null,
               "source_candidate_id":null,"usage_count":1,"last_used_at":null,
               "expires_at":null,"revoked_at":null,"deleted_at":null,
               "created_at":"2026-05-18T00:00:00Z","status":"approved"},
              {"id":"mc_2","workspace_id":"ws_default","text":"b","category":"y",
               "sensitivity":"medium","confidence":0.6,"scope":null,
               "source_candidate_id":null,"usage_count":null,"last_used_at":null,
               "expires_at":null,"revoked_at":null,"deleted_at":null,
               "created_at":"2026-05-18T01:00:00Z","status":"pending"},
              {"id":"mc_3","workspace_id":"ws_default","text":"c","category":"z",
               "sensitivity":"high","confidence":0.4,"scope":null,
               "source_candidate_id":null,"usage_count":null,"last_used_at":null,
               "expires_at":null,"revoked_at":null,"deleted_at":null,
               "created_at":"2026-05-18T02:00:00Z","status":"rejected"}
            ]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let rows = try await self.makeClient().memories(workspaceID: "ws_default", status: "all")
            #expect(rows.count == 3)
            // Discriminator order
            switch rows[0] { case .approved(let m): #expect(m.id == "mem_1"); default: Issue.record("not approved") }
            switch rows[1] { case .pending(let c):  #expect(c.id == "mc_2");  default: Issue.record("not pending")  }
            switch rows[2] { case .rejected(let c): #expect(c.id == "mc_3");  default: Issue.record("not rejected") }
        }
    }

    @Test("memoryConflicts() GETs /v1/memories/conflicts and decodes rows")
    func memoryConflicts() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/memories/conflicts")
            #expect(request.httpMethod == "GET")
            let resp = """
            {"conflicts":[{"id":"c_1","workspace_id":"ws","memory_a_id":"a",
            "memory_b_id":"b","conflict_type":"contradiction",
            "resolution_policy":"prefer_newer","last_resolved_at":null,
            "detected_at":"2026-05-18T00:00:00Z"}]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let cs = try await self.makeClient().memoryConflicts(workspaceID: "ws")
            #expect(cs.count == 1)
            #expect(cs[0].resolutionPolicy == "prefer_newer")
        }
    }

    // MARK: - Permissions

    @Test("permissions() GETs /v1/permissions and decodes AuthorityScopeRow")
    func permissionsList() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/permissions")
            #expect(request.httpMethod == "GET")
            let resp = """
            {"permissions":[{
              "id":"auth_1","workspace_id":"ws","actor_id":"act_user",
              "permission":"shell.run","resource_pattern":"echo *",
              "sensitivity_ceiling":"low","allowed_actions":["read","run"],
              "granted_by_actor_id":"act_admin","expires_at":null,
              "revoked_at":null,"created_at":"2026-05-18T00:00:00Z"
            }]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let ps = try await self.makeClient().permissions(workspaceID: "ws")
            #expect(ps.count == 1)
            #expect(ps[0].permission == "shell.run")
            #expect(ps[0].sensitivityCeiling == .low)
            #expect(ps[0].allowedActions.arrayValue?.count == 2)
        }
    }

    @Test("grantPermission() POSTs /v1/permissions and returns the new id")
    func grantPermission() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/permissions")
            #expect(request.httpMethod == "POST")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["workspace_id"] as? String == "ws")
            #expect(body["actor_id"]     as? String == "act_user")
            #expect(body["permission"]   as? String == "fs.read")
            #expect(body["level"]        as? String == "low")
            #expect(body["scope"]        as? String == "/tmp/*")
            let actions = body["allowed_actions"] as? [String]
            #expect(actions == ["read"])
            return (200, [:], Data(#"{"id":"auth_42"}"#.utf8))
        }) {
            let id = try await self.makeClient().grantPermission(
                workspaceID: "ws", actorID: "act_user",
                permission: "fs.read", level: "low",
                scope: "/tmp/*", allowedActions: ["read"]
            )
            #expect(id == "auth_42")
        }
    }

    @Test("revokePermission() DELETEs /v1/permissions with a JSON body")
    func revokePermission() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/permissions")
            #expect(request.httpMethod == "DELETE")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["workspace_id"]       as? String == "ws")
            #expect(body["authority_scope_id"] as? String == "auth_42")
            return (200, [:], Data(#"{"ok":true}"#.utf8))
        }) {
            try await self.makeClient().revokePermission(
                workspaceID: "ws", authorityScopeID: "auth_42"
            )
        }
    }

    // MARK: - Setup reports

    @Test("saveSetupReport() POSTs content and returns artifact+event IDs")
    func saveSetupReport() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/setup-reports")
            #expect(request.httpMethod == "POST")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["workspace_id"] as? String == "ws")
            #expect(body["actor_id"]     as? String == "act_user")
            #expect(body["content"]      as? String == "# setup\nstep 1")
            let resp = #"{"artifact_id":"art_1","event_id":"evt_1"}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let r = try await self.makeClient().saveSetupReport(
                workspaceID: "ws", actorID: "act_user", content: "# setup\nstep 1"
            )
            #expect(r.artifactID == "art_1")
            #expect(r.eventID    == "evt_1")
        }
    }

    @Test("latestSetupReport() GETs with latest=true and decodes wrapped report")
    func latestSetupReport() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/setup-reports")
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
                .queryItems ?? []
            #expect(qs.first(where: { $0.name == "latest" })?.value == "true")
            let resp = """
            {"report":{
              "artifact_id":"art_1","event_id":"evt_1",
              "content":"# setup","content_hash":"abc","bytes":7,
              "created_at":"2026-05-18T00:00:00Z"
            }}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let report = try await self.makeClient().latestSetupReport(workspaceID: "ws")
            #expect(report?.artifactID == "art_1")
            #expect(report?.contentHash == "abc")
            #expect(report?.bytes == 7)
        }
    }

    @Test("setupReports() returns reports without content_hash/bytes for list shape")
    func setupReportsList() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/setup-reports")
            // latest is NOT set
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
                .queryItems ?? []
            #expect(qs.first(where: { $0.name == "latest" }) == nil)
            let resp = """
            {"reports":[
              {"artifact_id":"art_1","event_id":"evt_1","content":"v1",
               "created_at":"2026-05-18T00:00:00Z"},
              {"artifact_id":"art_2","event_id":"evt_2","content":"v2",
               "created_at":"2026-05-18T01:00:00Z"}
            ]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let reports = try await self.makeClient().setupReports(workspaceID: "ws")
            #expect(reports.count == 2)
            #expect(reports[0].contentHash == nil) // list-shape omits it
            #expect(reports[1].content == "v2")
        }
    }

    // MARK: - Scout records

    @Test("saveScoutRecord() POSTs full body and returns IDs")
    func saveScoutRecord() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/scout-records")
            #expect(request.httpMethod == "POST")
            let body = try! JSONSerialization.jsonObject(with: request.bodyData()) as! [String: Any]
            #expect(body["source_id"]   as? String == "src_browser")
            #expect(body["kind"]        as? String == "page_snapshot")
            #expect(body["sensitivity"] as? String == "low")
            #expect(body["content"]     as? String == "<html>")
            let resp = #"{"artifact_id":"art_9","event_id":"evt_9"}"#
            return (200, [:], Data(resp.utf8))
        }) {
            let r = try await self.makeClient().saveScoutRecord(
                workspaceID: "ws", actorID: "act_user",
                sourceID: "src_browser", kind: "page_snapshot",
                sensitivity: .low, content: "<html>",
                metadata: .object(["url": .string("https://example.com")])
            )
            #expect(r.artifactID == "art_9")
        }
    }

    @Test("scoutRecords() forwards optional source filter and decodes rows")
    func scoutRecordsFiltered() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/scout-records")
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
                .queryItems ?? []
            #expect(qs.first(where: { $0.name == "workspace_id" })?.value == "ws")
            #expect(qs.first(where: { $0.name == "source" })?.value == "src_browser")
            let resp = """
            {"records":[{
              "artifact_id":"art_9","event_id":"evt_9",
              "source_id":"src_browser","kind":"page_snapshot",
              "sensitivity":"low","content":"<html>",
              "metadata":{"url":"https://example.com"},
              "created_at":"2026-05-18T00:00:00Z"
            }]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let rows = try await self.makeClient().scoutRecords(
                workspaceID: "ws", source: "src_browser"
            )
            #expect(rows.count == 1)
            #expect(rows[0].sourceID == "src_browser")
            #expect(rows[0].metadata["url"]?.stringValue == "https://example.com")
        }
    }
}
