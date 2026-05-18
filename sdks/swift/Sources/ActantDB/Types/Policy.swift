import Foundation

// MARK: - Policy document

public struct ToolRiskEntry: Codable, Sendable {
    public let tool: String
    public let risk: Risk
    public let requireApproval: Bool

    public init(tool: String, risk: Risk, requireApproval: Bool = false) {
        self.tool = tool; self.risk = risk; self.requireApproval = requireApproval
    }

    enum CodingKeys: String, CodingKey {
        case tool, risk
        case requireApproval = "require_approval"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.tool = try c.decode(String.self, forKey: .tool)
        self.risk = try c.decode(Risk.self, forKey: .risk)
        self.requireApproval = try c.decodeIfPresent(Bool.self, forKey: .requireApproval) ?? false
    }
}

public struct ArgDenyRule: Codable, Sendable {
    public let tool: String
    public let pattern: String
    public let reason: String

    public init(tool: String, pattern: String, reason: String) {
        self.tool = tool; self.pattern = pattern; self.reason = reason
    }
}

public struct Policy: Codable, Sendable {
    public let tools: [ToolRiskEntry]
    public let deny: [ArgDenyRule]
    public let sensitivityCeiling: Sensitivity?
    public let label: String

    public init(
        tools: [ToolRiskEntry] = [],
        deny: [ArgDenyRule] = [],
        sensitivityCeiling: Sensitivity? = nil,
        label: String = ""
    ) {
        self.tools = tools; self.deny = deny
        self.sensitivityCeiling = sensitivityCeiling; self.label = label
    }

    enum CodingKeys: String, CodingKey {
        case tools, deny
        case sensitivityCeiling = "sensitivity_ceiling"
        case label
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.tools = try c.decodeIfPresent([ToolRiskEntry].self, forKey: .tools) ?? []
        self.deny  = try c.decodeIfPresent([ArgDenyRule].self,  forKey: .deny)  ?? []
        self.sensitivityCeiling = try c.decodeIfPresent(Sensitivity.self, forKey: .sensitivityCeiling)
        self.label = try c.decodeIfPresent(String.self, forKey: .label) ?? ""
    }
}

// MARK: - PolicyVerdict (internally-tagged union, custom Codable)
//
// Rust shape: `#[serde(tag = "decision", rename_all = "snake_case")]`
// JSON: { "decision": "allow", "reason": "...", "policy_snapshot": "..." }

public enum PolicyVerdict: Codable, Sendable, Equatable {
    case allow(reason: String, policySnapshot: String)
    case constrain(reason: String, policySnapshot: String,
                   constrainedInput: JSONValue, hint: String)
    case requireApproval(reason: String, policySnapshot: String,
                         hint: String?, constrainedInput: JSONValue?)
    case block(reason: String, policySnapshot: String)
    case halt(reason: String, policySnapshot: String)

    /// Stable wire kind (matches Rust `PolicyVerdict::kind()`).
    public var kind: String {
        switch self {
        case .allow:           return "allow"
        case .constrain:       return "constrain"
        case .requireApproval: return "require_approval"
        case .block:           return "block"
        case .halt:            return "halt"
        }
    }

    private enum K: String, CodingKey {
        case decision, reason
        case policySnapshot   = "policy_snapshot"
        case constrainedInput = "constrained_input"
        case hint
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        let decision = try c.decode(String.self, forKey: .decision)
        let reason   = try c.decode(String.self, forKey: .reason)
        let snap     = try c.decode(String.self, forKey: .policySnapshot)
        switch decision {
        case "allow":
            self = .allow(reason: reason, policySnapshot: snap)
        case "constrain":
            let input = try c.decode(JSONValue.self, forKey: .constrainedInput)
            let hint  = try c.decode(String.self,    forKey: .hint)
            self = .constrain(reason: reason, policySnapshot: snap,
                              constrainedInput: input, hint: hint)
        case "require_approval":
            let hint  = try c.decodeIfPresent(String.self,    forKey: .hint)
            let input = try c.decodeIfPresent(JSONValue.self, forKey: .constrainedInput)
            self = .requireApproval(reason: reason, policySnapshot: snap,
                                    hint: hint, constrainedInput: input)
        case "block":
            self = .block(reason: reason, policySnapshot: snap)
        case "halt":
            self = .halt(reason: reason, policySnapshot: snap)
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .decision, in: c,
                debugDescription: "unknown PolicyVerdict decision: \(decision)"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: K.self)
        try c.encode(kind, forKey: .decision)
        switch self {
        case let .allow(reason, snap), let .block(reason, snap), let .halt(reason, snap):
            try c.encode(reason, forKey: .reason)
            try c.encode(snap,   forKey: .policySnapshot)
        case let .constrain(reason, snap, input, hint):
            try c.encode(reason, forKey: .reason)
            try c.encode(snap,   forKey: .policySnapshot)
            try c.encode(input,  forKey: .constrainedInput)
            try c.encode(hint,   forKey: .hint)
        case let .requireApproval(reason, snap, hint, input):
            try c.encode(reason, forKey: .reason)
            try c.encode(snap,   forKey: .policySnapshot)
            try c.encodeIfPresent(hint,  forKey: .hint)
            try c.encodeIfPresent(input, forKey: .constrainedInput)
        }
    }
}

// MARK: - Approvals

public struct ApprovalRequest: Codable, Sendable {
    public let toolCallID: String
    public let tool: String
    public let args: JSONValue
    public let hint: String?
    public let constrainedInput: JSONValue?
    public let reason: String

    enum CodingKeys: String, CodingKey {
        case toolCallID       = "tool_call_id"
        case tool, args, hint
        case constrainedInput = "constrained_input"
        case reason
    }
}

/// Approval outcome. Wire: `{"decision": "approve" | "approve_constrained" | "deny", ...}`.
public enum ApprovalDecisionV: Codable, Sendable, Equatable {
    case approve(approver: String, scope: String)
    case approveConstrained(approver: String, scope: String, acceptedInput: JSONValue)
    case deny(approver: String, reason: String)

    public var kind: String {
        switch self {
        case .approve:            return "approve"
        case .approveConstrained: return "approve_constrained"
        case .deny:               return "deny"
        }
    }

    private enum K: String, CodingKey {
        case decision, approver, scope, reason
        case acceptedInput = "accepted_input"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: K.self)
        let decision = try c.decode(String.self, forKey: .decision)
        let approver = try c.decode(String.self, forKey: .approver)
        switch decision {
        case "approve":
            let scope = try c.decode(String.self, forKey: .scope)
            self = .approve(approver: approver, scope: scope)
        case "approve_constrained":
            let scope = try c.decode(String.self, forKey: .scope)
            let input = try c.decode(JSONValue.self, forKey: .acceptedInput)
            self = .approveConstrained(approver: approver, scope: scope, acceptedInput: input)
        case "deny":
            let reason = try c.decode(String.self, forKey: .reason)
            self = .deny(approver: approver, reason: reason)
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .decision, in: c,
                debugDescription: "unknown ApprovalDecision: \(decision)"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: K.self)
        try c.encode(kind, forKey: .decision)
        switch self {
        case let .approve(approver, scope):
            try c.encode(approver, forKey: .approver)
            try c.encode(scope,    forKey: .scope)
        case let .approveConstrained(approver, scope, input):
            try c.encode(approver, forKey: .approver)
            try c.encode(scope,    forKey: .scope)
            try c.encode(input,    forKey: .acceptedInput)
        case let .deny(approver, reason):
            try c.encode(approver, forKey: .approver)
            try c.encode(reason,   forKey: .reason)
        }
    }
}

/// Pending-approval row returned by `GET /v1/approvals?workspace_id=...`.
/// Distinct from `ApprovalRequest` (Guard's contract payload).
public struct PendingApproval: Codable, Sendable, Identifiable {
    public let id: String
    public let toolCallID: String
    public let requestedBy: String
    public let riskLevel: String
    public let summary: String
    public let status: String

    enum CodingKeys: String, CodingKey {
        case id
        case toolCallID  = "tool_call_id"
        case requestedBy = "requested_by"
        case riskLevel   = "risk_level"
        case summary, status
    }
}
