import Foundation
import ActantDB

/// High-level wrapper around the propose/approve/reject memory commands and
/// the `GET /v1/memories` query endpoints.
public struct MemoryStore: Sendable {
    public let backend: AgentBackend

    public init(backend: AgentBackend) {
        self.backend = backend
    }

    /// Dispatch `propose_memory`. Returns the new candidate ID extracted
    /// from the command's `result.candidate_id` field (if the server
    /// returns it; otherwise the command ID).
    @discardableResult
    public func propose(
        text: String,
        category: String,
        sensitivity: Sensitivity,
        confidence: Double,
        evidence: JSONValue
    ) async throws -> String {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let actorID     = backend.actorID

        let r = try await client.proposeMemory(
            workspaceID: workspaceID, actorID: actorID,
            text: text, category: category,
            sensitivity: sensitivity, confidence: confidence,
            evidence: evidence
        )
        if let id = r.result["candidate_id"]?.stringValue {
            return id
        }
        if let id = r.result["memory_id"]?.stringValue {
            return id
        }
        return r.commandID
    }

    public func approve(candidateID: String) async throws {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let actorID     = backend.actorID

        _ = try await client.approveMemory(
            workspaceID: workspaceID, actorID: actorID, candidateID: candidateID
        )
    }

    public func reject(candidateID: String, reason: String?) async throws {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let actorID     = backend.actorID

        _ = try await client.rejectMemory(
            workspaceID: workspaceID, actorID: actorID,
            candidateID: candidateID, reason: reason
        )
    }

    /// All approved memories for the workspace.
    public func listApproved() async throws -> [ApprovedMemory] {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let rows = try await client.memories(workspaceID: workspaceID, status: "approved")
        return rows.compactMap {
            if case .approved(let m) = $0 { return m } else { return nil }
        }
    }

    /// All pending memory candidates for the workspace.
    public func listPending() async throws -> [MemoryCandidate] {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let rows = try await client.memories(workspaceID: workspaceID, status: "pending")
        return rows.compactMap {
            if case .pending(let c) = $0 { return c } else { return nil }
        }
    }

    /// Active memory conflicts in the workspace.
    public func listConflicts() async throws -> [MemoryConflict] {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        return try await client.memoryConflicts(workspaceID: workspaceID)
    }
}
