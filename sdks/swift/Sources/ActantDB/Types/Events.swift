import Foundation

// MARK: - Sensitivity / Risk / status enums
// Match Rust serde renames: lowercase for Sensitivity/Risk/CausalityKind/ToolCallStatus,
// snake_case for EventKind. RawValues are the wire strings.

public enum Sensitivity: String, Codable, Sendable, CaseIterable {
    case `public`
    case low
    case medium
    case high
    case secret
}

public enum Risk: String, Codable, Sendable, CaseIterable {
    case low
    case medium
    case high
    case destructive
}

public enum CausalityKind: String, Codable, Sendable, CaseIterable {
    case observation
    case intent
    case effect
    case control
    case audit
}

public enum ToolCallStatus: String, Codable, Sendable, CaseIterable {
    case ok
    case error
    case blocked
    case denied
    case replayed
}

/// Causal kind for each event written to the ledger.
/// Wire format is snake_case (e.g. `tool_call_completed`).
public enum EventKind: String, Codable, Sendable, CaseIterable {
    case agentRunStarted        = "agent_run_started"
    case userMessageReceived    = "user_message_received"
    case modelCall              = "model_call"
    case toolCallRequested      = "tool_call_requested"
    case guardVerdict           = "guard_verdict"
    case approvalRequired       = "approval_required"
    case approvalDecision       = "approval_decision"
    case toolCallStarted        = "tool_call_started"
    case toolCallCompleted      = "tool_call_completed"
    case contextBuild           = "context_build"
    case effectObserved         = "effect_observed"
    case agentRunFinished       = "agent_run_finished"
}

// MARK: - actant_contracts::ActantEvent (replay / studio shape)

/// Spec-level event from `actant-contracts`. Carries a typed `kind` and a hash-
/// chained payload. Distinct from `AgentEvent` (the storage-row shape returned
/// by `/v1/events`).
public struct ActantEvent: Codable, Sendable, Identifiable {
    public let id: String
    public let kind: EventKind
    public let project: String
    public let runID: String
    public let parentEventID: String?
    public let payload: JSONValue
    public let payloadHash: String
    public let chainHash: String
    public let sensitivity: Sensitivity
    public let createdAt: String

    enum CodingKeys: String, CodingKey {
        case id, kind, project
        case runID         = "run_id"
        case parentEventID = "parent_event_id"
        case payload
        case payloadHash   = "payload_hash"
        case chainHash     = "chain_hash"
        case sensitivity
        case createdAt     = "created_at"
    }
}

// MARK: - actant_core::AgentEvent (storage-row shape)

/// One row from the `agent_event` table — what `/v1/events?session_id=...`
/// returns. The `eventType` field is a stringly-typed enum (see actant spec
/// 04); compare against `EventKind.rawValue` for typed dispatch.
public struct AgentEvent: Codable, Sendable, Identifiable {
    public let id: String
    public let workspaceID: String
    public let actorID: String
    public let sessionID: String?
    public let parentEventID: String?
    public let eventType: String
    public let causalityKind: CausalityKind
    public let sensitivity: Sensitivity
    public let authorityScopeID: String?
    public let payloadInline: String?
    public let payloadRef: String?
    public let payloadHash: String
    public let eventHash: String
    public let createdAt: String
    public let modelCallID: String?
    public let toolCallID: String?
    public let workflowRunID: String?
    public let memoryID: String?
    public let artifactID: String?
    public let commandID: String?
    public let effectID: String?

    enum CodingKeys: String, CodingKey {
        case id
        case workspaceID      = "workspace_id"
        case actorID          = "actor_id"
        case sessionID        = "session_id"
        case parentEventID    = "parent_event_id"
        case eventType        = "event_type"
        case causalityKind    = "causality_kind"
        case sensitivity
        case authorityScopeID = "authority_scope_id"
        case payloadInline    = "payload_inline"
        case payloadRef       = "payload_ref"
        case payloadHash      = "payload_hash"
        case eventHash        = "event_hash"
        case createdAt        = "created_at"
        case modelCallID      = "model_call_id"
        case toolCallID       = "tool_call_id"
        case workflowRunID    = "workflow_run_id"
        case memoryID         = "memory_id"
        case artifactID       = "artifact_id"
        case commandID        = "command_id"
        case effectID         = "effect_id"
    }

    /// Parse `payloadInline` as a `JSONValue`. Returns nil if the field is
    /// absent or empty; throws on malformed JSON.
    public func parsedPayload() throws -> JSONValue? {
        guard let s = payloadInline, !s.isEmpty,
              let data = s.data(using: .utf8) else { return nil }
        return try JSONDecoder().decode(JSONValue.self, from: data)
    }
}

// MARK: - Context manifest / model call / tool call payloads

public struct ContextItem: Codable, Sendable, Identifiable {
    public let id: String
    public let kind: String
    public let source: String
    public let contentHash: String
    public let sensitivity: Sensitivity
    public let label: String
    public let flags: [String]

    public init(
        id: String, kind: String, source: String, contentHash: String,
        sensitivity: Sensitivity, label: String, flags: [String] = []
    ) {
        self.id = id; self.kind = kind; self.source = source
        self.contentHash = contentHash; self.sensitivity = sensitivity
        self.label = label; self.flags = flags
    }

    enum CodingKeys: String, CodingKey {
        case id, kind, source
        case contentHash = "content_hash"
        case sensitivity, label, flags
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.id = try c.decode(String.self, forKey: .id)
        self.kind = try c.decode(String.self, forKey: .kind)
        self.source = try c.decode(String.self, forKey: .source)
        self.contentHash = try c.decode(String.self, forKey: .contentHash)
        self.sensitivity = try c.decode(Sensitivity.self, forKey: .sensitivity)
        self.label = try c.decode(String.self, forKey: .label)
        self.flags = try c.decodeIfPresent([String].self, forKey: .flags) ?? []
    }
}

public struct ContextManifest: Codable, Sendable {
    public let manifestHash: String
    public let included: [ContextItem]
    public let blocked: [ContextItem]

    public init(manifestHash: String, included: [ContextItem], blocked: [ContextItem] = []) {
        self.manifestHash = manifestHash; self.included = included; self.blocked = blocked
    }

    enum CodingKeys: String, CodingKey {
        case manifestHash = "manifest_hash"
        case included, blocked
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.manifestHash = try c.decode(String.self, forKey: .manifestHash)
        self.included = try c.decode([ContextItem].self, forKey: .included)
        self.blocked = try c.decodeIfPresent([ContextItem].self, forKey: .blocked) ?? []
    }
}

public struct ModelCall: Codable, Sendable {
    public let model: String
    public let role: String
    public let promptHash: String
    public let tokensIn: UInt32?
    public let tokensOut: UInt32?
    public let summary: String

    enum CodingKeys: String, CodingKey {
        case model, role
        case promptHash = "prompt_hash"
        case tokensIn   = "tokens_in"
        case tokensOut  = "tokens_out"
        case summary
    }
}

public struct ToolCallRequest: Codable, Sendable {
    public let toolCallID: String
    public let tool: String
    public let args: JSONValue
    public let risk: Risk

    enum CodingKeys: String, CodingKey {
        case toolCallID = "tool_call_id"
        case tool, args, risk
    }
}

public struct ToolCallCompleted: Codable, Sendable {
    public let toolCallID: String
    public let status: ToolCallStatus
    public let result: JSONValue
    public let durationMS: UInt64

    enum CodingKeys: String, CodingKey {
        case toolCallID = "tool_call_id"
        case status, result
        case durationMS = "duration_ms"
    }
}
