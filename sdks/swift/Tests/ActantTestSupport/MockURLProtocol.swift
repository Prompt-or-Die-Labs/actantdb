import Foundation

/// URLProtocol stub for tests. Tests use the async `with(_:_:)` helper to
/// acquire a cross-suite barrier, install a handler, run their body, and
/// release the barrier. The barrier is necessary because Swift Testing's
/// `.serialized` trait only serializes within a suite — the static handler
/// would otherwise race across suites.
///
/// Lives in the shared `ActantTestSupport` target so both `ActantDBTests`
/// and `ActantAgentTests` import the same impl — previously this file was
/// byte-duplicated in both test targets.
public final class MockURLProtocol: URLProtocol, @unchecked Sendable {

    public typealias Handler = @Sendable (URLRequest) -> (Int, [String: String], Data)

    private static let lock = NSLock()
    nonisolated(unsafe) private static var _handler: Handler?

    /// Run `body` with the given mock handler installed. Other suites awaiting
    /// the same mutex will queue up and run serially.
    public static func with<R: Sendable>(
        _ handler: @escaping Handler,
        body: @Sendable () async throws -> R
    ) async throws -> R {
        await Mutex.shared.acquire()
        lock.withLock { _handler = handler }
        defer {
            lock.withLock { _handler = nil }
            Task { await Mutex.shared.release() }
        }
        return try await body()
    }

    public static func makeSession() -> URLSession {
        let cfg = URLSessionConfiguration.ephemeral
        cfg.protocolClasses = [MockURLProtocol.self]
        return URLSession(configuration: cfg)
    }

    public override class func canInit(with request: URLRequest) -> Bool { true }
    public override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    public override func startLoading() {
        let handler = Self.lock.withLock { Self._handler }
        guard let handler else {
            client?.urlProtocol(self, didFailWithError: NSError(
                domain: "MockURLProtocol", code: -1,
                userInfo: [NSLocalizedDescriptionKey: "no handler set"]
            ))
            return
        }
        let (status, headers, body) = handler(request)
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: status,
            httpVersion: "HTTP/1.1",
            headerFields: headers
        )!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: body)
        client?.urlProtocolDidFinishLoading(self)
    }

    public override func stopLoading() {}
}

/// Tiny actor-based mutex. Used to serialize MockURLProtocol handler
/// ownership across test suites.
private actor Mutex {
    static let shared = Mutex()
    private var locked = false
    private var waiters: [CheckedContinuation<Void, Never>] = []

    func acquire() async {
        if !locked { locked = true; return }
        await withCheckedContinuation { (c: CheckedContinuation<Void, Never>) in
            waiters.append(c)
        }
        // resumed → lock is "handed off" to us, no further state change needed
    }

    func release() {
        if let next = waiters.first {
            waiters.removeFirst()
            next.resume()
            // lock stays true; ownership transferred
        } else {
            locked = false
        }
    }
}
