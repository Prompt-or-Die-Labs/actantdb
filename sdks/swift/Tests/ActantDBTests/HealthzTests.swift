import Foundation
import Testing
@testable import ActantDB

@Suite("Healthz")
struct HealthzTests {

    @Test("healthz() parses {status, time}")
    func parsesHealthz() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.url?.path == "/v1/healthz")
            #expect(request.httpMethod == "GET")
            let body = #"{"status":"ok","time":"2026-05-18T12:00:00Z"}"#.data(using: .utf8)!
            return (200, ["content-type": "application/json"], body)
        }) {
            let client = ActantClient(
                baseURL: URL(string: "http://127.0.0.1:4555")!,
                urlSession: MockURLProtocol.makeSession()
            )
            let h = try await client.healthz()
            #expect(h.status == "ok")
            #expect(h.time == "2026-05-18T12:00:00Z")
        }
    }

    @Test("non-2xx HTTP surfaces as ActantError.http with parsed kind+message")
    func surfacesHTTPError() async throws {
        try await MockURLProtocol.with({ _ in
            let body = #"{"error":"not_found","message":"workspace not found"}"#.data(using: .utf8)!
            return (404, ["content-type": "application/json"], body)
        }) {
            let client = ActantClient(
                baseURL: URL(string: "http://127.0.0.1:4555")!,
                urlSession: MockURLProtocol.makeSession()
            )
            do {
                _ = try await client.healthz()
                Issue.record("expected throw")
            } catch let ActantError.http(status, kind, message, _) {
                #expect(status == 404)
                #expect(kind == "not_found")
                #expect(message == "workspace not found")
            }
        }
    }

    @Test("Bearer token is sent in Authorization header when provided")
    func sendsBearerToken() async throws {
        try await MockURLProtocol.with({ request in
            #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer test-token")
            let body = #"{"status":"ok","time":"2026-05-18T12:00:00Z"}"#.data(using: .utf8)!
            return (200, [:], body)
        }) {
            let client = ActantClient(
                baseURL: URL(string: "http://127.0.0.1:4555")!,
                token: "test-token",
                urlSession: MockURLProtocol.makeSession()
            )
            _ = try await client.healthz()
        }
    }

    @Test("healthz readiness probe with ok=true returns isHealthy")
    func readinessProbe() async throws {
        try await MockURLProtocol.with({ _ in
            let body = #"{"phase":"ready","ok":true}"#.data(using: .utf8)!
            return (200, [:], body)
        }) {
            let client = ActantClient(
                baseURL: URL(string: "http://127.0.0.1:4555")!,
                urlSession: MockURLProtocol.makeSession()
            )
            let h = try await client.healthzReady()
            #expect(h.phase == "ready")
            #expect(h.isHealthy)
        }
    }
}
