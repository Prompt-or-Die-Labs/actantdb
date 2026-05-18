import Foundation

public struct CheckpointRef: Codable, Sendable {
    public let eventID: String
    public let runID: String
    public let manifestHash: String
    public let policyHash: String
    public let memorySetHash: String
    public let priorToolResults: [String]

    public init(
        eventID: String, runID: String,
        manifestHash: String, policyHash: String,
        memorySetHash: String, priorToolResults: [String] = []
    ) {
        self.eventID = eventID; self.runID = runID
        self.manifestHash = manifestHash; self.policyHash = policyHash
        self.memorySetHash = memorySetHash; self.priorToolResults = priorToolResults
    }

    enum CodingKeys: String, CodingKey {
        case eventID          = "event_id"
        case runID            = "run_id"
        case manifestHash     = "manifest_hash"
        case policyHash       = "policy_hash"
        case memorySetHash    = "memory_set_hash"
        case priorToolResults = "prior_tool_results"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.eventID = try c.decode(String.self, forKey: .eventID)
        self.runID   = try c.decode(String.self, forKey: .runID)
        self.manifestHash  = try c.decode(String.self, forKey: .manifestHash)
        self.policyHash    = try c.decode(String.self, forKey: .policyHash)
        self.memorySetHash = try c.decode(String.self, forKey: .memorySetHash)
        self.priorToolResults = try c.decodeIfPresent([String].self, forKey: .priorToolResults) ?? []
    }
}

public struct ReplayOverrides: Codable, Sendable {
    public let policy: String?
    public let withoutMemory: [String]
    public let model: String?

    public init(policy: String? = nil, withoutMemory: [String] = [], model: String? = nil) {
        self.policy = policy; self.withoutMemory = withoutMemory; self.model = model
    }

    enum CodingKeys: String, CodingKey {
        case policy
        case withoutMemory = "without_memory"
        case model
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.policy = try c.decodeIfPresent(String.self, forKey: .policy)
        self.withoutMemory = try c.decodeIfPresent([String].self, forKey: .withoutMemory) ?? []
        self.model = try c.decodeIfPresent(String.self, forKey: .model)
    }
}

public struct ReplayRun: Codable, Sendable, Identifiable {
    public let id: String
    public let fromEvent: String
    public let originalRun: String
    public let overrides: ReplayOverrides
    public let createdAt: String
    public let events: [ActantEvent]

    enum CodingKeys: String, CodingKey {
        case id
        case fromEvent    = "from_event"
        case originalRun  = "original_run"
        case overrides
        case createdAt    = "created_at"
        case events
    }
}

public enum DiffKind: String, Codable, Sendable, CaseIterable {
    case identical
    case changed
    case missing
    case extra
}

public struct DiffEntry: Codable, Sendable {
    public let kind: String
    public let diff: DiffKind
    public let a: JSONValue?
    public let b: JSONValue?
}

public struct ReplayDiff: Codable, Sendable {
    public let a: String
    public let b: String
    public let entries: [DiffEntry]
}

/// Replay execution mode (server-side enum).
public enum ReplayMode: String, Codable, Sendable, CaseIterable {
    case recorded
    case model
    case policy
    case memory
}
