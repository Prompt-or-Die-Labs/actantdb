import Foundation
import ActantDB

/// Workspace-scoped pending-approvals view + approve/deny actions. Thin
/// wrapper over `ActantClient.approvals(...)`, `approveToolCall(...)`,
/// `denyToolCall(...)` and a `dispatch(...)`-based constrained-approval.
public struct ApprovalCenter: Sendable {
    public let backend: AgentBackend

    public init(backend: AgentBackend) {
        self.backend = backend
    }

    public func pending() async throws -> [PendingApproval] {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        return try await client.approvals(workspaceID: workspaceID)
    }

    public func approve(toolCallID: String, scope: String) async throws {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let actorID     = backend.actorID
        _ = try await client.approveToolCall(
            workspaceID: workspaceID, actorID: actorID,
            toolCallID: toolCallID, scope: scope
        )
    }

    public func deny(toolCallID: String, reason: String) async throws {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let actorID     = backend.actorID
        _ = try await client.denyToolCall(
            workspaceID: workspaceID, actorID: actorID,
            toolCallID: toolCallID, reason: reason
        )
    }

    /// Approve a tool call while overriding the original input with
    /// `acceptedInput`. Dispatches `approve_tool_call` with the
    /// `accepted_input` field added — the server accepts this today even
    /// though it's not a dedicated command, and it's forward-compatible
    /// with a future `approve_with_constraint`.
    public func approveConstrained(
        toolCallID: String,
        acceptedInput: JSONValue,
        scope: String
    ) async throws {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let actorID     = backend.actorID

        _ = try await client.dispatch(
            workspaceID: workspaceID, actorID: actorID,
            commandType: "approve_tool_call",
            input: .object([
                "tool_call_id":   .string(toolCallID),
                "scope":          .string(scope),
                "accepted_input": acceptedInput,
            ])
        )
    }
}
