import Foundation
import Testing
@testable import ActantAgent

@Suite("AgentBackend.waitForReady")
struct BackendTests {

    @Test("waitForReady succeeds after a 503-then-200 sequence")
    func waitsThroughTransientFailure() async throws {
        resetCalls()
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/healthz/ready")
            let n = nextCall()
            if n == 1 {
                let body = #"{"error":"not_ready","message":"warming up"}"#
                return (503, [:], Data(body.utf8))
            } else {
                let body = #"{"phase":"ready","ok":true}"#
                return (200, [:], Data(body.utf8))
            }
        }) {
            let backend = makeBackend()
            try await backend.waitForReady(timeout: 5.0)
            #expect(callsObserved() >= 2)
        }
    }

    @Test("waitForReady throws .transport when deadline elapses")
    func timesOut() async throws {
        try await MockURLProtocol.with({ _ in
            let body = #"{"error":"not_ready","message":"warming up"}"#
            return (503, [:], Data(body.utf8))
        }) {
            let backend = makeBackend()
            do {
                try await backend.waitForReady(timeout: 0.15)
                Issue.record("expected throw")
            } catch let ActantError.transport(msg) {
                #expect(msg.contains("server not ready"))
            }
        }
    }
}

// MARK: - cross-call sequencer (mutex-isolated; only used inside `MockURLProtocol.with`)

nonisolated(unsafe) private var _calls: Int = 0
private let _callsLock = NSLock()

func nextCall() -> Int {
    _callsLock.lock(); defer { _callsLock.unlock() }
    _calls += 1
    return _calls
}

func callsObserved() -> Int {
    _callsLock.lock(); defer { _callsLock.unlock() }
    return _calls
}

func resetCalls() {
    _callsLock.lock(); defer { _callsLock.unlock() }
    _calls = 0
}
