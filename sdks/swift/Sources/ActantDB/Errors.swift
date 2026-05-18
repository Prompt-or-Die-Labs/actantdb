import Foundation

/// Errors surfaced by `ActantClient`. The `http` case mirrors the server's
/// error key (see `actant-server::err_response`):
///   - `invalid_input`      → 400
///   - `not_found`          → 404
///   - `permission_denied`  → 403
///   - `approval_required`  → 202   (NOT a failure on the server, but treated
///                                   as a typed signal here)
///   - `approval_denied`    → 403
///   - `idempotent_replay`  → 200   (typed signal; body decodes normally)
///   - `internal`           → 500
///   - `rate_limited`       → 429   (sent by `dispatch_command` with retry-after)
///   - `missing_authorization` / `invalid_token` / `workspace_mismatch` → 401/403
public enum ActantError: Error, Sendable, CustomStringConvertible {
    case http(status: Int, kind: String, message: String, body: Data)
    case transport(String)
    case invalidURL(String)
    case decoding(String, body: Data)
    case webSocket(String)
    case cancelled

    public var description: String {
        switch self {
        case let .http(status, kind, message, _):
            return "ActantError.http(\(status) \(kind)): \(message)"
        case let .transport(m):     return "ActantError.transport: \(m)"
        case let .invalidURL(u):    return "ActantError.invalidURL: \(u)"
        case let .decoding(m, _):   return "ActantError.decoding: \(m)"
        case let .webSocket(m):     return "ActantError.webSocket: \(m)"
        case .cancelled:            return "ActantError.cancelled"
        }
    }

    static func from(status: Int, body: Data) -> ActantError {
        struct ErrBody: Decodable {
            let error: String?
            let message: String?
        }
        let parsed = (try? JSONDecoder().decode(ErrBody.self, from: body)) ?? ErrBody(error: nil, message: nil)
        let kind = parsed.error ?? "http_\(status)"
        let message = parsed.message ?? String(data: body, encoding: .utf8) ?? ""
        return .http(status: status, kind: kind, message: message, body: body)
    }
}
