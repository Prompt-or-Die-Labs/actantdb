import Foundation
import ActantDB

public enum RuntimeGoalStatus: String, Codable, Sendable, CaseIterable {
    case active
    case completed
    case archived
}

public struct RuntimeGoal: Codable, Sendable, Identifiable, Equatable {
    public let id: String
    public let title: String
    public let status: RuntimeGoalStatus
    public let createdAt: String
    public let updatedAt: String
    public let metadata: JSONValue

    public init(
        id: String,
        title: String,
        status: RuntimeGoalStatus = .active,
        createdAt: String,
        updatedAt: String,
        metadata: JSONValue = .object([:])
    ) {
        self.id = id
        self.title = title
        self.status = status
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.metadata = metadata
    }

    enum CodingKeys: String, CodingKey {
        case id, title, status, metadata
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

public struct ManifestationRecord: Codable, Sendable, Identifiable, Equatable {
    public let id: String
    public let goalID: String?
    public let summary: String
    public let payload: JSONValue
    public let createdAt: String

    public init(
        id: String,
        goalID: String? = nil,
        summary: String,
        payload: JSONValue = .object([:]),
        createdAt: String
    ) {
        self.id = id
        self.goalID = goalID
        self.summary = summary
        self.payload = payload
        self.createdAt = createdAt
    }

    enum CodingKeys: String, CodingKey {
        case id, summary, payload
        case goalID = "goal_id"
        case createdAt = "created_at"
    }
}

public struct ScoutState: Codable, Sendable, Equatable {
    public let cursor: String?
    public let updatedAt: String?
    public let sources: [String: JSONValue]

    public init(
        cursor: String? = nil,
        updatedAt: String? = nil,
        sources: [String: JSONValue] = [:]
    ) {
        self.cursor = cursor
        self.updatedAt = updatedAt
        self.sources = sources
    }

    enum CodingKeys: String, CodingKey {
        case cursor, sources
        case updatedAt = "updated_at"
    }
}

public struct WorkflowDraft: Codable, Sendable, Identifiable, Equatable {
    public let id: String
    public let title: String
    public let payload: JSONValue
    public let createdAt: String
    public let updatedAt: String

    public init(
        id: String,
        title: String,
        payload: JSONValue = .object([:]),
        createdAt: String,
        updatedAt: String
    ) {
        self.id = id
        self.title = title
        self.payload = payload
        self.createdAt = createdAt
        self.updatedAt = updatedAt
    }

    enum CodingKeys: String, CodingKey {
        case id, title, payload
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

public struct RuntimeStateSnapshot: Codable, Sendable, Equatable {
    public let version: Int
    public var goals: [RuntimeGoal]
    public var manifestationHistory: [ManifestationRecord]
    public var scoutState: ScoutState
    public var workflowDrafts: [WorkflowDraft]

    public init(
        version: Int = 1,
        goals: [RuntimeGoal] = [],
        manifestationHistory: [ManifestationRecord] = [],
        scoutState: ScoutState = ScoutState(),
        workflowDrafts: [WorkflowDraft] = []
    ) {
        self.version = version
        self.goals = goals
        self.manifestationHistory = manifestationHistory
        self.scoutState = scoutState
        self.workflowDrafts = workflowDrafts
    }

    enum CodingKeys: String, CodingKey {
        case version, goals
        case manifestationHistory = "manifestation_history"
        case scoutState = "scout_state"
        case workflowDrafts = "workflow_drafts"
    }
}

public actor FileBackedRuntimeStateStore {
    public let fileURL: URL
    private var cache: RuntimeStateSnapshot?

    public init(fileURL: URL) {
        self.fileURL = fileURL
    }

    public func load() throws -> RuntimeStateSnapshot {
        if let cache { return cache }
        guard FileManager.default.fileExists(atPath: fileURL.path) else {
            let empty = RuntimeStateSnapshot()
            cache = empty
            return empty
        }
        let data = try Data(contentsOf: fileURL)
        let snapshot = try Self.decoder.decode(RuntimeStateSnapshot.self, from: data)
        cache = snapshot
        return snapshot
    }

    public func replace(_ snapshot: RuntimeStateSnapshot) throws {
        cache = snapshot
        try persist(snapshot)
    }

    public func upsertGoal(_ goal: RuntimeGoal) throws {
        var snapshot = try load()
        if let index = snapshot.goals.firstIndex(where: { $0.id == goal.id }) {
            snapshot.goals[index] = goal
        } else {
            snapshot.goals.append(goal)
        }
        try replace(snapshot)
    }

    public func appendManifestation(_ record: ManifestationRecord) throws {
        var snapshot = try load()
        snapshot.manifestationHistory.append(record)
        try replace(snapshot)
    }

    public func updateScoutState(_ scoutState: ScoutState) throws {
        var snapshot = try load()
        snapshot.scoutState = scoutState
        try replace(snapshot)
    }

    public func upsertWorkflowDraft(_ draft: WorkflowDraft) throws {
        var snapshot = try load()
        if let index = snapshot.workflowDrafts.firstIndex(where: { $0.id == draft.id }) {
            snapshot.workflowDrafts[index] = draft
        } else {
            snapshot.workflowDrafts.append(draft)
        }
        try replace(snapshot)
    }

    public func deleteWorkflowDraft(id: String) throws {
        var snapshot = try load()
        snapshot.workflowDrafts.removeAll { $0.id == id }
        try replace(snapshot)
    }

    private func persist(_ snapshot: RuntimeStateSnapshot) throws {
        try FileManager.default.createDirectory(
            at: fileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let data = try Self.encoder.encode(snapshot)
        try data.write(to: fileURL, options: [.atomic])
    }

    private static let encoder: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes]
        return encoder
    }()

    private static let decoder = JSONDecoder()
}
