import Foundation

/// Alpha command types registered by `actant-server`. Mirrors
/// `GET /v1/metadata/commands`. Use the rawValue when calling `dispatch(...)`.
public enum CommandType: String, Codable, Sendable, CaseIterable {
    case createSession      = "create_session"
    case appendUserMessage  = "append_user_message"
    case appendAgentMessage = "append_agent_message"
    case requestToolCall    = "request_tool_call"
    case approveToolCall    = "approve_tool_call"
    case denyToolCall       = "deny_tool_call"
    case recordToolResult   = "record_tool_result"
    case proposeMemory      = "propose_memory"
    case approveMemory      = "approve_memory"
    case rejectMemory       = "reject_memory"
}

/// Body for `POST /v1/command`.
public struct CommandRequest: Codable, Sendable {
    public let workspaceID: String
    public let actorID: String
    public let commandType: String
    public let input: JSONValue
    public let idempotencyKey: String?

    public init(
        workspaceID: String,
        actorID: String,
        commandType: String,
        input: JSONValue,
        idempotencyKey: String? = nil
    ) {
        self.workspaceID = workspaceID
        self.actorID = actorID
        self.commandType = commandType
        self.input = input
        self.idempotencyKey = idempotencyKey
    }

    enum CodingKeys: String, CodingKey {
        case workspaceID    = "workspace_id"
        case actorID        = "actor_id"
        case commandType    = "command_type"
        case input
        case idempotencyKey = "idempotency_key"
    }
}

/// Response from `POST /v1/command`. Result shape varies per command — keep
/// as `JSONValue` and let callers unwrap via convenience methods.
public struct CommandResponse: Codable, Sendable {
    public let commandID: String
    public let eventID: String?
    public let result: JSONValue

    enum CodingKeys: String, CodingKey {
        case commandID = "command_id"
        case eventID   = "event_id"
        case result
    }
}

// MARK: - Server metadata + sync responses

public struct CommandsMetadata: Codable, Sendable {
    public let commands: [String]
}

public struct EventsResponse: Codable, Sendable {
    public let events: [AgentEvent]
}

public struct ApprovalsResponse: Codable, Sendable {
    public let approvals: [PendingApproval]
}

public struct SyncSinceResponse: Codable, Sendable {
    public let events: [SyncEvent]
    public let nextSince: String?

    enum CodingKeys: String, CodingKey {
        case events
        case nextSince = "next_since"
    }
}

/// Slim event row returned by `POST /v1/sync/since`. Lighter than `AgentEvent`
/// — only the fields the cluster-sync endpoint emits.
public struct SyncEvent: Codable, Sendable, Identifiable {
    public let id: String
    public let eventType: String
    public let actorID: String
    public let payloadHash: String
    public let payloadInline: String?
    public let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id
        case eventType     = "event_type"
        case actorID       = "actor_id"
        case payloadHash   = "payload_hash"
        case payloadInline = "payload_inline"
        case createdAt     = "created_at"
    }
}

public struct CheckpointResponse: Codable, Sendable {
    public let checkpointID: String

    enum CodingKeys: String, CodingKey {
        case checkpointID = "checkpoint_id"
    }
}
