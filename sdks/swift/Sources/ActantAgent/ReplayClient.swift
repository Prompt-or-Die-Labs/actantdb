import Foundation
import ActantDB

/// Replay facade — pure wrappers over `ActantClient.replayCheckpoint(...)`
/// and `ActantClient.replayRun(...)`. The actor ID is passed explicitly
/// (matching the underlying client signature) so callers can run a replay
/// under an actor different from `backend.actorID` if they need to.
public struct ReplayClient: Sendable {
    public let backend: AgentBackend

    public init(backend: AgentBackend) {
        self.backend = backend
    }

    public func checkpoint(eventID: String) async throws -> String {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        return try await client.replayCheckpoint(
            workspaceID: workspaceID, eventID: eventID
        )
    }

    public func run(
        actorID: String,
        checkpointID: String,
        mode: ReplayMode
    ) async throws -> ReplayDiff {
        let client = backend.client
        return try await client.replayRun(
            actorID: actorID, checkpointID: checkpointID, mode: mode
        )
    }
}
