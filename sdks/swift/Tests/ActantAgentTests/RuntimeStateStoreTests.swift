import Foundation
import Testing
@testable import ActantAgent

@Suite("RuntimeStateStore")
struct RuntimeStateStoreTests {
    @Test("missing state file loads an empty snapshot")
    func missingFileLoadsEmptySnapshot() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("actant-runtime-state-\(UUID().uuidString)")
        let store = FileBackedRuntimeStateStore(fileURL: dir.appendingPathComponent("state.json"))
        let snapshot = try await store.load()
        #expect(snapshot.version == 1)
        #expect(snapshot.goals.isEmpty)
        #expect(snapshot.manifestationHistory.isEmpty)
        #expect(snapshot.workflowDrafts.isEmpty)
        #expect(snapshot.scoutState == ScoutState())
    }

    @Test("goals manifestations scout state and workflow drafts survive a new store")
    func persistedStateSurvivesRestart() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("actant-runtime-state-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = dir.appendingPathComponent("state.json")
        let now = "2026-05-20T00:00:00Z"
        let first = FileBackedRuntimeStateStore(fileURL: file)

        try await first.upsertGoal(
            RuntimeGoal(
                id: "goal_1",
                title: "ship embedded runtime",
                createdAt: now,
                updatedAt: now,
                metadata: .object(["priority": .string("high")])
            )
        )
        try await first.appendManifestation(
            ManifestationRecord(
                id: "manifest_1",
                goalID: "goal_1",
                summary: "created local persistence",
                payload: .object(["event": .string("write")]),
                createdAt: now
            )
        )
        try await first.updateScoutState(
            ScoutState(
                cursor: "cursor_1",
                updatedAt: now,
                sources: ["repo": .object(["path": .string("/Users/home/actantDB")])]
            )
        )
        try await first.upsertWorkflowDraft(
            WorkflowDraft(
                id: "draft_1",
                title: "runtime-state workflow",
                payload: .object(["step": .string("persist")]),
                createdAt: now,
                updatedAt: now
            )
        )

        let second = FileBackedRuntimeStateStore(fileURL: file)
        let snapshot = try await second.load()
        #expect(snapshot.goals.map(\.id) == ["goal_1"])
        #expect(snapshot.goals[0].metadata["priority"]?.stringValue == "high")
        #expect(snapshot.manifestationHistory.map(\.id) == ["manifest_1"])
        #expect(snapshot.scoutState.cursor == "cursor_1")
        #expect(snapshot.scoutState.sources["repo"]?["path"]?.stringValue == "/Users/home/actantDB")
        #expect(snapshot.workflowDrafts.map(\.id) == ["draft_1"])
    }

    @Test("workflow draft deletion is persisted")
    func workflowDraftDeletionPersists() async throws {
        let dir = FileManager.default.temporaryDirectory
            .appendingPathComponent("actant-runtime-state-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let file = dir.appendingPathComponent("state.json")
        let store = FileBackedRuntimeStateStore(fileURL: file)
        let now = "2026-05-20T00:00:00Z"

        try await store.upsertWorkflowDraft(
            WorkflowDraft(id: "draft_1", title: "draft", createdAt: now, updatedAt: now)
        )
        try await store.deleteWorkflowDraft(id: "draft_1")

        let second = FileBackedRuntimeStateStore(fileURL: file)
        let snapshot = try await second.load()
        #expect(snapshot.workflowDrafts.isEmpty)
    }
}
