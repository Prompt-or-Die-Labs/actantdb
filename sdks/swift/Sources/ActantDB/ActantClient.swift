import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// HTTP client for an ActantDB server (Rust `actantdb serve`).
///
/// Covers every endpoint registered by `actant-server::router`:
///   GET  /v1/healthz, /v1/healthz/{startup,live,ready}
///   GET  /v1/metadata/commands, /v1/openapi.yaml, /v1/metrics
///   POST /v1/command
///   GET  /v1/events?session_id=...
///   GET  /v1/approvals?workspace_id=...
///   POST /v1/replay/checkpoint, /v1/replay/run
///   POST /v1/sync/since
///   GET  /v1/ws  (see `Subscribe.swift`)
public struct ActantClient: Sendable {
    public let baseURL: URL
    public let tokenProvider: any TokenProvider
    public let timeout: TimeInterval
    public let urlSession: URLSession

    /// Static-token convenience initializer (the typical case).
    public init(
        baseURL: URL,
        token: String? = nil,
        timeout: TimeInterval = 10,
        urlSession: URLSession = .shared
    ) {
        self.init(
            baseURL: baseURL,
            tokenProvider: StaticToken(token),
            timeout: timeout,
            urlSession: urlSession
        )
    }

    /// Custom-provider initializer (for OIDC refresh, etc.).
    public init(
        baseURL: URL,
        tokenProvider: any TokenProvider,
        timeout: TimeInterval = 10,
        urlSession: URLSession = .shared
    ) {
        self.baseURL = baseURL
        self.tokenProvider = tokenProvider
        self.timeout = timeout
        self.urlSession = urlSession
    }

    // MARK: - Health

    public func healthz() async throws -> Healthz {
        try await request("GET", path: "/v1/healthz")
    }

    public func healthzStartup() async throws -> Healthz {
        try await request("GET", path: "/v1/healthz/startup")
    }

    public func healthzLive() async throws -> Healthz {
        try await request("GET", path: "/v1/healthz/live")
    }

    public func healthzReady() async throws -> Healthz {
        try await request("GET", path: "/v1/healthz/ready")
    }

    // MARK: - Metadata

    public func metadataCommands() async throws -> CommandsMetadata {
        try await request("GET", path: "/v1/metadata/commands")
    }

    public func openapi() async throws -> String {
        let (data, _) = try await rawRequest("GET", path: "/v1/openapi.yaml")
        return String(data: data, encoding: .utf8) ?? ""
    }

    public func metrics() async throws -> String {
        let (data, _) = try await rawRequest("GET", path: "/v1/metrics")
        return String(data: data, encoding: .utf8) ?? ""
    }

    // MARK: - Command dispatch

    /// Generic command dispatch. Prefer the typed convenience methods below
    /// unless you need to call a command this SDK version doesn't know about.
    public func dispatch(
        workspaceID: String,
        actorID: String,
        commandType: String,
        input: JSONValue,
        idempotencyKey: String? = nil
    ) async throws -> CommandResponse {
        let body = CommandRequest(
            workspaceID: workspaceID,
            actorID: actorID,
            commandType: commandType,
            input: input,
            idempotencyKey: idempotencyKey
        )
        return try await request("POST", path: "/v1/command", body: body)
    }

    /// Typed-command overload.
    public func dispatch(
        workspaceID: String,
        actorID: String,
        command: CommandType,
        input: JSONValue,
        idempotencyKey: String? = nil
    ) async throws -> CommandResponse {
        try await dispatch(
            workspaceID: workspaceID,
            actorID: actorID,
            commandType: command.rawValue,
            input: input,
            idempotencyKey: idempotencyKey
        )
    }

    // MARK: - Command convenience methods

    /// Create a new session. Returns the new `session_id`.
    @discardableResult
    public func createSession(
        workspaceID: String,
        actorID: String,
        title: String? = nil
    ) async throws -> String {
        var input: [String: JSONValue] = [:]
        if let title { input["title"] = .string(title) }
        let r = try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .createSession, input: .object(input)
        )
        guard let id = r.result["session_id"]?.stringValue else {
            throw ActantError.decoding("create_session result missing session_id",
                                       body: Data())
        }
        return id
    }

    @discardableResult
    public func appendUserMessage(
        workspaceID: String, actorID: String,
        sessionID: String, text: String
    ) async throws -> CommandResponse {
        try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .appendUserMessage,
            input: ["session_id": .string(sessionID), "text": .string(text)]
        )
    }

    @discardableResult
    public func appendAgentMessage(
        workspaceID: String, actorID: String,
        sessionID: String, text: String
    ) async throws -> CommandResponse {
        try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .appendAgentMessage,
            input: ["session_id": .string(sessionID), "text": .string(text)]
        )
    }

    @discardableResult
    public func requestToolCall(
        workspaceID: String, actorID: String,
        sessionID: String, toolName: String, arguments: JSONValue
    ) async throws -> CommandResponse {
        try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .requestToolCall,
            input: [
                "session_id": .string(sessionID),
                "tool_name":  .string(toolName),
                "arguments":  arguments,
            ]
        )
    }

    @discardableResult
    public func approveToolCall(
        workspaceID: String, actorID: String,
        toolCallID: String, scope: String = "once"
    ) async throws -> CommandResponse {
        try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .approveToolCall,
            input: ["tool_call_id": .string(toolCallID), "scope": .string(scope)]
        )
    }

    @discardableResult
    public func denyToolCall(
        workspaceID: String, actorID: String,
        toolCallID: String, reason: String
    ) async throws -> CommandResponse {
        try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .denyToolCall,
            input: ["tool_call_id": .string(toolCallID), "reason": .string(reason)]
        )
    }

    @discardableResult
    public func recordToolResult(
        workspaceID: String, actorID: String,
        toolCallID: String, result: JSONValue
    ) async throws -> CommandResponse {
        try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .recordToolResult,
            input: ["tool_call_id": .string(toolCallID), "result": result]
        )
    }

    @discardableResult
    public func proposeMemory(
        workspaceID: String, actorID: String,
        text: String, category: String,
        sensitivity: Sensitivity = .low,
        confidence: Double = 1.0,
        evidence: JSONValue = .null
    ) async throws -> CommandResponse {
        try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .proposeMemory,
            input: [
                "text":        .string(text),
                "category":    .string(category),
                "sensitivity": .string(sensitivity.rawValue),
                "confidence":  .double(confidence),
                "evidence":    evidence,
            ]
        )
    }

    @discardableResult
    public func approveMemory(
        workspaceID: String, actorID: String, candidateID: String
    ) async throws -> CommandResponse {
        try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .approveMemory,
            input: ["candidate_id": .string(candidateID)]
        )
    }

    @discardableResult
    public func rejectMemory(
        workspaceID: String, actorID: String,
        candidateID: String, reason: String? = nil
    ) async throws -> CommandResponse {
        var input: [String: JSONValue] = ["candidate_id": .string(candidateID)]
        if let reason { input["reason"] = .string(reason) }
        return try await dispatch(
            workspaceID: workspaceID, actorID: actorID,
            command: .rejectMemory, input: .object(input)
        )
    }

    // MARK: - Queries

    /// `GET /v1/events?session_id=...`. Returns AgentEvent rows (storage shape).
    public func events(sessionID: String) async throws -> [AgentEvent] {
        let r: EventsResponse = try await request(
            "GET", path: "/v1/events",
            query: [URLQueryItem(name: "session_id", value: sessionID)]
        )
        return r.events
    }

    public func approvals(workspaceID: String) async throws -> [PendingApproval] {
        let r: ApprovalsResponse = try await request(
            "GET", path: "/v1/approvals",
            query: [URLQueryItem(name: "workspace_id", value: workspaceID)]
        )
        return r.approvals
    }

    // MARK: - Replay

    public func replayCheckpoint(workspaceID: String, eventID: String) async throws -> String {
        struct Body: Encodable {
            let workspace_id: String
            let event_id: String
        }
        let r: CheckpointResponse = try await request(
            "POST", path: "/v1/replay/checkpoint",
            body: Body(workspace_id: workspaceID, event_id: eventID)
        )
        return r.checkpointID
    }

    public func replayRun(
        actorID: String, checkpointID: String, mode: ReplayMode
    ) async throws -> ReplayDiff {
        struct Body: Encodable {
            let actor_id: String
            let checkpoint_id: String
            let mode: String
        }
        return try await request(
            "POST", path: "/v1/replay/run",
            body: Body(actor_id: actorID, checkpoint_id: checkpointID, mode: mode.rawValue)
        )
    }

    // MARK: - Sync

    public func syncSince(
        workspaceID: String, sinceEventID: String = "", limit: UInt32 = 1000
    ) async throws -> SyncSinceResponse {
        struct Body: Encodable {
            let workspace_id: String
            let since_event_id: String
            let limit: UInt32
        }
        return try await request(
            "POST", path: "/v1/sync/since",
            body: Body(
                workspace_id: workspaceID,
                since_event_id: sinceEventID,
                limit: limit
            )
        )
    }

    // MARK: - Internal request plumbing

    func request<T: Decodable & Sendable>(
        _ method: String,
        path: String,
        query: [URLQueryItem] = [],
        body: Encodable? = nil
    ) async throws -> T {
        let (data, _) = try await rawRequest(method, path: path, query: query, body: body)
        if T.self == EmptyResponse.self {
            return EmptyResponse() as! T
        }
        do {
            return try JSONDecoder.actant.decode(T.self, from: data)
        } catch {
            throw ActantError.decoding(String(describing: error), body: data)
        }
    }

    func rawRequest(
        _ method: String,
        path: String,
        query: [URLQueryItem] = [],
        body: Encodable? = nil
    ) async throws -> (Data, HTTPURLResponse) {
        var components = URLComponents(
            url: baseURL.appendingPathComponent(path),
            resolvingAgainstBaseURL: false
        ) ?? URLComponents()
        if !query.isEmpty {
            components.queryItems = (components.queryItems ?? []) + query
        }
        guard let url = components.url else {
            throw ActantError.invalidURL(baseURL.absoluteString + path)
        }
        var req = URLRequest(url: url, timeoutInterval: timeout)
        req.httpMethod = method
        req.setValue("application/json", forHTTPHeaderField: "content-type")
        req.setValue("application/json", forHTTPHeaderField: "accept")
        if let token = try await tokenProvider.token() {
            req.setValue("Bearer \(token)", forHTTPHeaderField: "authorization")
        }
        if let body {
            req.httpBody = try JSONEncoder.actant.encode(AnyEncodable(body))
        }

        let (data, response): (Data, URLResponse)
        do {
            (data, response) = try await urlSession.data(for: req)
        } catch is CancellationError {
            throw ActantError.cancelled
        } catch {
            throw ActantError.transport(error.localizedDescription)
        }
        guard let http = response as? HTTPURLResponse else {
            throw ActantError.transport("non-HTTP response")
        }
        // The server's err_response wire shape is `{"error":"<kind>","message":"..."}`
        // and it's used for some 2xx codes too (202 approval_required,
        // 200 idempotent_replay). Treat any response carrying a top-level
        // `error` field as a typed failure, regardless of status code.
        struct ErrPeek: Decodable { let error: String? }
        if let peek = try? JSONDecoder().decode(ErrPeek.self, from: data),
           peek.error != nil {
            throw ActantError.from(status: http.statusCode, body: data)
        }
        guard (200..<300).contains(http.statusCode) else {
            throw ActantError.from(status: http.statusCode, body: data)
        }
        return (data, http)
    }
}

// MARK: - Response types

public struct Healthz: Decodable, Sendable {
    /// "ok" for /v1/healthz; phase string ("startup", "live", "ready") for the others.
    public let status: String?
    public let phase: String?
    public let ok: Bool?
    public let time: String?
    public let error: String?

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.status = try c.decodeIfPresent(String.self, forKey: .status)
        self.phase  = try c.decodeIfPresent(String.self, forKey: .phase)
        self.ok     = try c.decodeIfPresent(Bool.self,   forKey: .ok)
        self.time   = try c.decodeIfPresent(String.self, forKey: .time)
        self.error  = try c.decodeIfPresent(String.self, forKey: .error)
    }

    /// True when the response indicates a healthy state.
    public var isHealthy: Bool {
        if let status, status == "ok" { return true }
        if let ok { return ok }
        return false
    }

    enum CodingKeys: String, CodingKey { case status, phase, ok, time, error }
}

public struct EmptyResponse: Decodable, Sendable {
    public init() {}
}

// MARK: - JSON helpers

extension JSONEncoder {
    static let actant: JSONEncoder = {
        let e = JSONEncoder()
        e.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return e
    }()
}

extension JSONDecoder {
    static let actant: JSONDecoder = {
        JSONDecoder()
    }()
}

/// Type-erased wrapper. Used inline within a single call — does not cross
/// isolation domains, so no @Sendable.
struct AnyEncodable: Encodable {
    let _encode: (Encoder) throws -> Void
    init<E: Encodable>(_ value: E) {
        self._encode = { try value.encode(to: $0) }
    }
    func encode(to encoder: Encoder) throws { try _encode(encoder) }
}
