import Foundation
import ActantDB

/// Re-export of `ActantDB.Sensitivity` so consumers only need
/// `import ActantAgent` to construct memory candidates.
public typealias Sensitivity = ActantDB.Sensitivity

/// Re-export of `ActantDB.JSONValue`. Same rationale.
public typealias JSONValue = ActantDB.JSONValue

/// Re-export of `ActantDB.PendingApproval`. Returned by `ApprovalCenter`.
public typealias PendingApproval = ActantDB.PendingApproval

/// Re-exports of the storage row types used by `MemoryStore`, etc., so
/// consumers only need `import ActantAgent`.
public typealias ApprovedMemory     = ActantDB.ApprovedMemory
public typealias MemoryCandidate    = ActantDB.MemoryCandidate
public typealias MemoryConflict     = ActantDB.MemoryConflict
public typealias MemoryRow          = ActantDB.MemoryRow
public typealias AuthorityScopeRow  = ActantDB.AuthorityScopeRow
public typealias SetupReportRow     = ActantDB.SetupReportRow
public typealias ScoutRecordRow     = ActantDB.ScoutRecordRow
public typealias SaveArtifactResponse = ActantDB.SaveArtifactResponse

/// Re-export of `ActantDB.ReplayDiff` / `ActantDB.ReplayMode`.
public typealias ReplayDiff = ActantDB.ReplayDiff
public typealias ReplayMode = ActantDB.ReplayMode

/// Re-export of `ActantDB.ActantError` so consumers can `catch ActantError`
/// without an extra import.
public typealias ActantError = ActantDB.ActantError

/// Shared backend configuration for the high-level facades (`Session`,
/// `MemoryStore`, `Auditor`, `ApprovalCenter`, `ReplayClient`).
///
/// Holds the underlying `ActantClient` plus the `workspace_id` / `actor_id`
/// every command needs. An `actor` so multiple facades may share one backend
/// across tasks without external locking.
public actor AgentBackend {
    public let client: ActantClient
    public let workspaceID: String
    public let actorID: String

    public init(client: ActantClient, workspaceID: String, actorID: String) {
        self.client = client
        self.workspaceID = workspaceID
        self.actorID = actorID
    }

    /// Block (with exponential backoff) until `/v1/healthz/ready` reports
    /// healthy or `timeout` elapses, whichever comes first.
    ///
    /// Backoff schedule: 50ms → 200ms → 500ms → 1s, capped at 1s. On timeout
    /// throws `ActantError.transport("server not ready after \(timeout)s")`.
    public func waitForReady(timeout: TimeInterval) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        let backoffsMS: [UInt64] = [50, 200, 500, 1000]
        var step = 0

        while true {
            do {
                let h = try await client.healthzReady()
                if h.isHealthy { return }
            } catch is CancellationError {
                throw ActantError.cancelled
            } catch {
                // Treat any transport/HTTP failure as "not ready yet" and
                // keep polling until the deadline.
            }

            if Date() >= deadline {
                throw ActantError.transport("server not ready after \(timeout)s")
            }

            let delayMS = backoffsMS[min(step, backoffsMS.count - 1)]
            step += 1
            try await Task.sleep(nanoseconds: delayMS * 1_000_000)
        }
    }
}
