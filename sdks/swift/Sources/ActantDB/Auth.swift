import Foundation

/// Source of Bearer tokens for `ActantClient`. The default is a static token
/// (`StaticToken`). Implement this protocol for refresh / OIDC flows in a
/// follow-up — keep the v0 surface intentionally narrow.
///
/// `token()` is async to leave room for a refresh round-trip without breaking
/// callers when that lands.
public protocol TokenProvider: Sendable {
    func token() async throws -> String?
}

/// Static Bearer token. Used by `ActantClient.init(baseURL:token:...)`.
public struct StaticToken: TokenProvider {
    public let value: String?
    public init(_ value: String?) { self.value = value }
    public func token() async throws -> String? { value }
}
