import Foundation
import Testing
@testable import ActantDB

@Suite("Entities + entity-relations endpoints")
struct EntitiesEndpointTests {

    private func makeClient() -> ActantClient {
        ActantClient(
            baseURL: URL(string: "http://127.0.0.1:4555")!,
            token: nil,
            urlSession: MockURLProtocol.makeSession()
        )
    }

    @Test("createEntity POSTs /v1/entities and returns new id")
    func createEntity() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/entities")
            #expect(request.httpMethod == "POST")
            let body = try? JSONSerialization.jsonObject(with: request.bodyData())
                as? [String: Any]
            #expect(body?["type"] as? String == "person")
            #expect(body?["canonical_name"] as? String == "Alice")
            #expect((body?["aliases"] as? [String])?.first == "A.")
            return (200, ["content-type": "application/json"], Data(#"{"id":"ent_x"}"#.utf8))
        }) {
            let id = try await self.makeClient().createEntity(
                workspaceID: "ws_default",
                type: "person",
                canonicalName: "Alice",
                aliases: ["A.", "Alice Smith"],
                sensitivity: .low
            )
            #expect(id == "ent_x")
        }
    }

    @Test("entities(type:) GETs /v1/entities with workspace_id+type and decodes rows")
    func listEntities() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/entities")
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
                .queryItems ?? []
            #expect(qs.first(where: { $0.name == "type" })?.value == "person")
            let resp = """
            {"entities":[
              {"id":"ent_1","workspace_id":"ws_default","type":"person",
               "canonical_name":"Alice","aliases":["A."],"sensitivity":"low",
               "source_events":[],"capsule_id":null,
               "created_at":"2026-05-18T00:00:00Z"}
            ]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let rows = try await self.makeClient().entities(
                workspaceID: "ws_default", type: "person"
            )
            #expect(rows.count == 1)
            #expect(rows[0].canonicalName == "Alice")
            #expect(rows[0].aliases == ["A."])
        }
    }

    @Test("linkEntities POSTs /v1/entity-relations with the full body")
    func linkEntities() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/entity-relations")
            #expect(request.httpMethod == "POST")
            let body = try? JSONSerialization.jsonObject(with: request.bodyData())
                as? [String: Any]
            #expect(body?["source_entity"] as? String == "ent_a")
            #expect(body?["target_entity"] as? String == "ent_b")
            #expect(body?["relation_type"] as? String == "knows")
            #expect((body?["confidence"] as? Double) == 0.8)
            return (200, [:], Data(#"{"id":"rel_x"}"#.utf8))
        }) {
            let id = try await self.makeClient().linkEntities(
                workspaceID: "ws_default",
                source: "ent_a",
                relation: "knows",
                target: "ent_b",
                confidence: 0.8
            )
            #expect(id == "rel_x")
        }
    }

    @Test("entityRelations(entity:) GETs /v1/entity-relations with entity filter")
    func entityRelations() async throws {
        try await MockURLProtocol.with({ request in
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
                .queryItems ?? []
            #expect(qs.first(where: { $0.name == "entity" })?.value == "ent_b")
            let resp = """
            {"relations":[
              {"id":"rel_1","workspace_id":"ws_default","source_entity":"ent_a",
               "relation_type":"knows","target_entity":"ent_b","confidence":0.8,
               "evidence_events":[]}
            ]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let rows = try await self.makeClient().entityRelations(
                workspaceID: "ws_default", entity: "ent_b"
            )
            #expect(rows.count == 1)
            #expect(rows[0].sourceEntity == "ent_a")
            #expect(rows[0].relationType == "knows")
        }
    }
}
