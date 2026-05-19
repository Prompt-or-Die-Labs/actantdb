import Foundation
import ActantTestSupport
import Testing
import ActantDB
@testable import ActantAgent

@Suite("RelationshipStore")
struct RelationshipStoreTests {

    @Test("upsertEntity POSTs /v1/entities and returns the id")
    func upsertEntity() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/entities")
            #expect(request.httpMethod == "POST")
            let body = try? JSONSerialization.jsonObject(with: request.bodyData())
                as? [String: Any]
            #expect(body?["canonical_name"] as? String == "Alice")
            return (200, [:], Data(#"{"id":"ent_new"}"#.utf8))
        }) {
            let store = RelationshipStore(backend: makeBackend())
            let id = try await store.upsertEntity(type: "person", canonicalName: "Alice")
            #expect(id == "ent_new")
        }
    }

    @Test("link POSTs /v1/entity-relations with confidence")
    func link() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/entity-relations")
            let body = try? JSONSerialization.jsonObject(with: request.bodyData())
                as? [String: Any]
            #expect(body?["source_entity"] as? String == "ent_a")
            #expect((body?["confidence"] as? Double) == 0.75)
            return (200, [:], Data(#"{"id":"rel_z"}"#.utf8))
        }) {
            let store = RelationshipStore(backend: makeBackend())
            let id = try await store.link(
                source: "ent_a", relation: "knows", target: "ent_b", confidence: 0.75
            )
            #expect(id == "rel_z")
        }
    }

    @Test("neighbors GETs /v1/entity-relations?entity=... and returns rows")
    func neighbors() async throws {
        try await MockURLProtocol.with({ request in
            let qs = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)!
                .queryItems ?? []
            #expect(qs.first(where: { $0.name == "entity" })?.value == "ent_b")
            let resp = """
            {"relations":[
              {"id":"rel_1","workspace_id":"ws_default","source_entity":"ent_a",
               "relation_type":"knows","target_entity":"ent_b","confidence":1.0,
               "evidence_events":[]},
              {"id":"rel_2","workspace_id":"ws_default","source_entity":"ent_b",
               "relation_type":"likes","target_entity":"ent_c","confidence":0.5,
               "evidence_events":[]}
            ]}
            """
            return (200, [:], Data(resp.utf8))
        }) {
            let store = RelationshipStore(backend: makeBackend())
            let rels = try await store.neighbors(of: "ent_b")
            #expect(rels.count == 2)
        }
    }
}
