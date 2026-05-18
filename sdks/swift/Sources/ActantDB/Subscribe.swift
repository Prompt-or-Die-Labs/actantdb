import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// One message delivered over the `/v1/ws` topic stream. The server publishes
/// arbitrary JSON via `actant_subscribe::SubscribeHub.publish`; the SDK
/// decodes it as a `JSONValue` and lets callers route by shape.
public struct SubscriptionMessage: Sendable {
    public let data: Data
    public let json: JSONValue

    /// Try to decode the message body as a specific `Decodable` type.
    public func decode<T: Decodable>(_ type: T.Type) throws -> T {
        try JSONDecoder.actant.decode(type, from: data)
    }
}

extension ActantClient {

    /// Open a WebSocket subscription against `/v1/ws`. Returns an
    /// `AsyncThrowingStream<SubscriptionMessage, Error>` — iterating with
    /// `for try await msg in stream { ... }` reads messages until the server
    /// closes the connection, an error occurs, or the consuming task is
    /// cancelled. Cancellation cleanly cancels the underlying URLSession
    /// WebSocket task.
    ///
    /// - Parameters:
    ///   - workspaceID: required. The topic's workspace.
    ///   - sessionID: optional. When set, scopes to a single session.
    ///   - kind: defaults to "events" (matches server `default_kind`).
    public func subscribe(
        workspaceID: String,
        sessionID: String? = nil,
        kind: String = "events"
    ) async throws -> AsyncThrowingStream<SubscriptionMessage, any Error> {
        guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
            throw ActantError.invalidURL(baseURL.absoluteString)
        }
        // ws:// or wss:// based on the base scheme.
        switch components.scheme?.lowercased() {
        case "https": components.scheme = "wss"
        case "http":  components.scheme = "ws"
        case "wss", "ws": break
        default:
            throw ActantError.invalidURL("unsupported base scheme for subscribe")
        }
        components.path = (components.path.hasSuffix("/") ? components.path : components.path) + "v1/ws"
        var items: [URLQueryItem] = [
            URLQueryItem(name: "workspace_id", value: workspaceID),
            URLQueryItem(name: "kind", value: kind),
        ]
        if let sessionID { items.append(URLQueryItem(name: "session_id", value: sessionID)) }
        components.queryItems = items
        guard let url = components.url else {
            throw ActantError.invalidURL(baseURL.absoluteString + "/v1/ws")
        }

        var req = URLRequest(url: url, timeoutInterval: timeout)
        if let token = try await tokenProvider.token() {
            req.setValue("Bearer \(token)", forHTTPHeaderField: "authorization")
        }

        let task = urlSession.webSocketTask(with: req)
        task.resume()

        return AsyncThrowingStream<SubscriptionMessage, any Error> { continuation in
            continuation.onTermination = { _ in
                task.cancel(with: .goingAway, reason: nil)
            }
            Task {
                while !Task.isCancelled {
                    do {
                        let frame = try await task.receive()
                        let data: Data
                        switch frame {
                        case .data(let d):
                            data = d
                        case .string(let s):
                            data = s.data(using: .utf8) ?? Data()
                        @unknown default:
                            continue
                        }
                        let json = (try? JSONDecoder.actant.decode(JSONValue.self, from: data)) ?? .null
                        continuation.yield(SubscriptionMessage(data: data, json: json))
                    } catch is CancellationError {
                        continuation.finish(throwing: ActantError.cancelled)
                        return
                    } catch {
                        // URLSession surfaces a graceful close as an error.
                        // Map it to a clean stream end.
                        let ns = error as NSError
                        if ns.domain == NSURLErrorDomain && ns.code == NSURLErrorCancelled {
                            continuation.finish()
                        } else {
                            continuation.finish(throwing: ActantError.webSocket(error.localizedDescription))
                        }
                        return
                    }
                }
                continuation.finish()
            }
        }
    }
}
