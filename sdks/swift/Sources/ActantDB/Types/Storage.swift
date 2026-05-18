import Foundation

// MARK: - Memory rows
//
// Mirrors the JSON shapes emitted by `GET /v1/memories?status=...` and
// `GET /v1/memories/conflicts` in actant-server. Approved-memory rows come
// from the `memory` table; pending/rejected rows come from `memory_candidate`
// and are emitted with the same envelope (with the candidate-only fields set
// to `null`). The `status` field on each row is the discriminator —
// "approved" | "pending" | "rejected".

/// Approved memory row (status="approved"). Originates from the `memory` table.
public struct ApprovedMemory: Codable, Sendable, Identifiable {
    public let id: String
    public let workspaceID: String
    public let text: String
    public let category: String
    public let sensitivity: Sensitivity
    public let confidence: Double?
    public let scope: String?
    public let sourceCandidateID: String?
    public let usageCount: Int64?
    public let lastUsedAt: String?
    public let expiresAt: String?
    public let revokedAt: String?
    public let deletedAt: String?
    public let createdAt: String
    /// Always "approved" for this struct, but kept on the wire shape so the
    /// `MemoryRow` discriminator round-trips.
    public let status: String

    public init(
        id: String,
        workspaceID: String,
        text: String,
        category: String,
        sensitivity: Sensitivity,
        confidence: Double? = nil,
        scope: String? = nil,
        sourceCandidateID: String? = nil,
        usageCount: Int64? = nil,
        lastUsedAt: String? = nil,
        expiresAt: String? = nil,
        revokedAt: String? = nil,
        deletedAt: String? = nil,
        createdAt: String,
        status: String = "approved"
    ) {
        self.id = id
        self.workspaceID = workspaceID
        self.text = text
        self.category = category
        self.sensitivity = sensitivity
        self.confidence = confidence
        self.scope = scope
        self.sourceCandidateID = sourceCandidateID
        self.usageCount = usageCount
        self.lastUsedAt = lastUsedAt
        self.expiresAt = expiresAt
        self.revokedAt = revokedAt
        self.deletedAt = deletedAt
        self.createdAt = createdAt
        self.status = status
    }

    enum CodingKeys: String, CodingKey {
        case id
        case workspaceID       = "workspace_id"
        case text, category, sensitivity, confidence, scope
        case sourceCandidateID = "source_candidate_id"
        case usageCount        = "usage_count"
        case lastUsedAt        = "last_used_at"
        case expiresAt         = "expires_at"
        case revokedAt         = "revoked_at"
        case deletedAt         = "deleted_at"
        case createdAt         = "created_at"
        case status
    }
}

/// Pending or rejected memory candidate row. Originates from the
/// `memory_candidate` table; `status` is "pending" or "rejected".
public struct MemoryCandidate: Codable, Sendable, Identifiable {
    public let id: String
    public let workspaceID: String
    public let text: String
    public let category: String
    public let sensitivity: Sensitivity
    public let confidence: Double
    public let createdAt: String
    public let status: String

    public init(
        id: String,
        workspaceID: String,
        text: String,
        category: String,
        sensitivity: Sensitivity,
        confidence: Double,
        createdAt: String,
        status: String
    ) {
        self.id = id
        self.workspaceID = workspaceID
        self.text = text
        self.category = category
        self.sensitivity = sensitivity
        self.confidence = confidence
        self.createdAt = createdAt
        self.status = status
    }

    enum CodingKeys: String, CodingKey {
        case id
        case workspaceID = "workspace_id"
        case text, category, sensitivity, confidence
        case createdAt   = "created_at"
        case status
    }
}

/// Discriminated union over the rows returned by `GET /v1/memories`. The
/// `status` JSON field selects the case.
public enum MemoryRow: Codable, Sendable, Identifiable {
    case approved(ApprovedMemory)
    case pending(MemoryCandidate)
    case rejected(MemoryCandidate)

    public var id: String {
        switch self {
        case .approved(let m): return m.id
        case .pending(let c):  return c.id
        case .rejected(let c): return c.id
        }
    }

    public var status: String {
        switch self {
        case .approved: return "approved"
        case .pending:  return "pending"
        case .rejected: return "rejected"
        }
    }

    private struct StatusPeek: Decodable {
        let status: String
    }

    public init(from decoder: Decoder) throws {
        let peek = try StatusPeek(from: decoder)
        switch peek.status {
        case "approved":
            self = .approved(try ApprovedMemory(from: decoder))
        case "pending":
            self = .pending(try MemoryCandidate(from: decoder))
        case "rejected":
            self = .rejected(try MemoryCandidate(from: decoder))
        default:
            throw DecodingError.dataCorrupted(.init(
                codingPath: decoder.codingPath,
                debugDescription: "unknown memory row status: \(peek.status)"
            ))
        }
    }

    public func encode(to encoder: Encoder) throws {
        switch self {
        case .approved(let m): try m.encode(to: encoder)
        case .pending(let c):  try c.encode(to: encoder)
        case .rejected(let c): try c.encode(to: encoder)
        }
    }
}

/// Row returned by `GET /v1/memories/conflicts`.
public struct MemoryConflict: Codable, Sendable, Identifiable {
    public let id: String
    public let workspaceID: String
    public let memoryAID: String
    public let memoryBID: String
    public let conflictType: String
    public let resolutionPolicy: String?
    public let lastResolvedAt: String?
    public let detectedAt: String

    public init(
        id: String,
        workspaceID: String,
        memoryAID: String,
        memoryBID: String,
        conflictType: String,
        resolutionPolicy: String? = nil,
        lastResolvedAt: String? = nil,
        detectedAt: String
    ) {
        self.id = id
        self.workspaceID = workspaceID
        self.memoryAID = memoryAID
        self.memoryBID = memoryBID
        self.conflictType = conflictType
        self.resolutionPolicy = resolutionPolicy
        self.lastResolvedAt = lastResolvedAt
        self.detectedAt = detectedAt
    }

    enum CodingKeys: String, CodingKey {
        case id
        case workspaceID      = "workspace_id"
        case memoryAID        = "memory_a_id"
        case memoryBID        = "memory_b_id"
        case conflictType     = "conflict_type"
        case resolutionPolicy = "resolution_policy"
        case lastResolvedAt   = "last_resolved_at"
        case detectedAt       = "detected_at"
    }
}

// MARK: - Permission rows

/// One row returned by `GET /v1/permissions`. Mirrors a non-revoked entry of
/// the `authority_scope` table.
public struct AuthorityScopeRow: Codable, Sendable, Identifiable {
    public let id: String
    public let workspaceID: String
    public let actorID: String
    public let permission: String
    public let resourcePattern: String?
    public let sensitivityCeiling: Sensitivity
    /// Server emits the `allowed_actions` column either as a JSON array or
    /// as a single-element fallback array of the raw blob — kept as
    /// `JSONValue` so neither case fails decode.
    public let allowedActions: JSONValue
    public let grantedByActorID: String?
    public let expiresAt: String?
    public let revokedAt: String?
    public let createdAt: String

    public init(
        id: String,
        workspaceID: String,
        actorID: String,
        permission: String,
        resourcePattern: String? = nil,
        sensitivityCeiling: Sensitivity,
        allowedActions: JSONValue,
        grantedByActorID: String? = nil,
        expiresAt: String? = nil,
        revokedAt: String? = nil,
        createdAt: String
    ) {
        self.id = id
        self.workspaceID = workspaceID
        self.actorID = actorID
        self.permission = permission
        self.resourcePattern = resourcePattern
        self.sensitivityCeiling = sensitivityCeiling
        self.allowedActions = allowedActions
        self.grantedByActorID = grantedByActorID
        self.expiresAt = expiresAt
        self.revokedAt = revokedAt
        self.createdAt = createdAt
    }

    enum CodingKeys: String, CodingKey {
        case id
        case workspaceID        = "workspace_id"
        case actorID            = "actor_id"
        case permission
        case resourcePattern    = "resource_pattern"
        case sensitivityCeiling = "sensitivity_ceiling"
        case allowedActions     = "allowed_actions"
        case grantedByActorID   = "granted_by_actor_id"
        case expiresAt          = "expires_at"
        case revokedAt          = "revoked_at"
        case createdAt          = "created_at"
    }
}

// MARK: - Setup report rows
//
// Used for both the list and the `latest=true` shape. The list shape lacks
// `content_hash` and `bytes`, so both are optional. The latest shape carries
// them.

public struct SetupReportRow: Codable, Sendable, Identifiable {
    public let artifactID: String
    public let eventID: String
    public let content: String
    public let contentHash: String?
    public let bytes: Int64?
    public let createdAt: String

    public var id: String { artifactID }

    public init(
        artifactID: String,
        eventID: String,
        content: String,
        contentHash: String? = nil,
        bytes: Int64? = nil,
        createdAt: String
    ) {
        self.artifactID = artifactID
        self.eventID = eventID
        self.content = content
        self.contentHash = contentHash
        self.bytes = bytes
        self.createdAt = createdAt
    }

    enum CodingKeys: String, CodingKey {
        case artifactID  = "artifact_id"
        case eventID     = "event_id"
        case content
        case contentHash = "content_hash"
        case bytes
        case createdAt   = "created_at"
    }
}

// MARK: - Scout record rows

public struct ScoutRecordRow: Codable, Sendable, Identifiable {
    public let artifactID: String
    public let eventID: String
    public let sourceID: String
    public let kind: String
    public let sensitivity: Sensitivity
    public let content: String
    public let metadata: JSONValue
    public let createdAt: String

    public var id: String { artifactID }

    public init(
        artifactID: String,
        eventID: String,
        sourceID: String,
        kind: String,
        sensitivity: Sensitivity,
        content: String,
        metadata: JSONValue,
        createdAt: String
    ) {
        self.artifactID = artifactID
        self.eventID = eventID
        self.sourceID = sourceID
        self.kind = kind
        self.sensitivity = sensitivity
        self.content = content
        self.metadata = metadata
        self.createdAt = createdAt
    }

    enum CodingKeys: String, CodingKey {
        case artifactID  = "artifact_id"
        case eventID     = "event_id"
        case sourceID    = "source_id"
        case kind, sensitivity, content, metadata
        case createdAt   = "created_at"
    }
}

// MARK: - Wire responses

public struct MemoriesResponse: Codable, Sendable {
    public let memories: [MemoryRow]
}

public struct MemoryConflictsResponse: Codable, Sendable {
    public let conflicts: [MemoryConflict]
}

public struct PermissionsResponse: Codable, Sendable {
    public let permissions: [AuthorityScopeRow]
}

public struct SetupReportsResponse: Codable, Sendable {
    public let reports: [SetupReportRow]
}

public struct SetupReportLatestResponse: Codable, Sendable {
    public let report: SetupReportRow?
}

public struct ScoutRecordsResponse: Codable, Sendable {
    public let records: [ScoutRecordRow]
}

/// Returned by `POST /v1/permissions`.
public struct GrantPermissionResponse: Codable, Sendable {
    public let id: String
}

/// Returned by `DELETE /v1/permissions`.
public struct RevokePermissionResponse: Codable, Sendable {
    public let ok: Bool
}

/// Shared response shape for `POST /v1/setup-reports` and
/// `POST /v1/scout-records` — both write an event + artifact and echo the IDs.
public struct SaveArtifactResponse: Codable, Sendable {
    public let artifactID: String
    public let eventID: String

    public init(artifactID: String, eventID: String) {
        self.artifactID = artifactID
        self.eventID = eventID
    }

    enum CodingKeys: String, CodingKey {
        case artifactID = "artifact_id"
        case eventID    = "event_id"
    }
}

// MARK: - Entities + relations

/// One `entity` row.
public struct EntityRow: Codable, Sendable, Identifiable {
    public let id: String
    public let workspaceID: String
    public let type: String
    public let canonicalName: String
    public let aliases: [String]
    public let sensitivity: Sensitivity
    public let sourceEvents: [String]
    public let capsuleID: String?
    public let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id, type, aliases, sensitivity
        case workspaceID   = "workspace_id"
        case canonicalName = "canonical_name"
        case sourceEvents  = "source_events"
        case capsuleID     = "capsule_id"
        case createdAt     = "created_at"
    }
}

/// One `entity_relation` row.
public struct EntityRelationRow: Codable, Sendable, Identifiable {
    public let id: String
    public let workspaceID: String
    public let sourceEntity: String
    public let relationType: String
    public let targetEntity: String
    public let confidence: Double
    public let evidenceEvents: [String]

    enum CodingKeys: String, CodingKey {
        case id, confidence
        case workspaceID    = "workspace_id"
        case sourceEntity   = "source_entity"
        case relationType   = "relation_type"
        case targetEntity   = "target_entity"
        case evidenceEvents = "evidence_events"
    }
}

public struct EntitiesResponse: Codable, Sendable {
    public let entities: [EntityRow]
}

public struct EntityRelationsResponse: Codable, Sendable {
    public let relations: [EntityRelationRow]
}

public struct CreateEntityResponse: Codable, Sendable {
    public let id: String
}

public struct CreateEntityRelationResponse: Codable, Sendable {
    public let id: String
}
