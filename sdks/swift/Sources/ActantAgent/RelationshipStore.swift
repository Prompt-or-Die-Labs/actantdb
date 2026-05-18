import Foundation
import ActantDB

/// High-level wrapper around the entity + entity-relation endpoints.
/// Lets a consumer treat ActantDB as a small directed knowledge graph:
/// nodes are `EntityRow`s (people, places, projects, …), edges are typed
/// `EntityRelationRow`s with a confidence score.
public struct RelationshipStore: Sendable {
    public let backend: AgentBackend

    public init(backend: AgentBackend) {
        self.backend = backend
    }

    /// Create a new entity. Returns the new entity ID (`ent_…`).
    @discardableResult
    public func upsertEntity(
        type: String,
        canonicalName: String,
        aliases: [String] = [],
        sensitivity: Sensitivity = .low,
        capsuleID: String? = nil,
        sourceEvents: [String] = []
    ) async throws -> String {
        try await backend.client.createEntity(
            workspaceID: backend.workspaceID,
            type: type,
            canonicalName: canonicalName,
            aliases: aliases,
            sensitivity: sensitivity,
            capsuleID: capsuleID,
            sourceEvents: sourceEvents
        )
    }

    /// Link two entities with a typed relation. Returns the new relation ID
    /// (`rel_…`).
    @discardableResult
    public func link(
        source: String,
        relation: String,
        target: String,
        confidence: Double = 1.0,
        evidenceEvents: [String] = []
    ) async throws -> String {
        try await backend.client.linkEntities(
            workspaceID: backend.workspaceID,
            source: source,
            relation: relation,
            target: target,
            confidence: confidence,
            evidenceEvents: evidenceEvents
        )
    }

    /// List entities in the workspace, optionally filtered by type.
    public func entities(type: String? = nil) async throws -> [EntityRow] {
        try await backend.client.entities(
            workspaceID: backend.workspaceID,
            type: type
        )
    }

    /// All relations incident to `entityID` (where it appears as source or
    /// target), optionally filtered by `relationType`. Single hop.
    public func neighbors(
        of entityID: String,
        relationType: String? = nil
    ) async throws -> [EntityRelationRow] {
        try await backend.client.entityRelations(
            workspaceID: backend.workspaceID,
            entity: entityID,
            relationType: relationType
        )
    }
}
