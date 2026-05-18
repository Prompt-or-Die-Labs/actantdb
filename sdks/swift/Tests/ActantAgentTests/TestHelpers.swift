import Foundation
import ActantDB
import ActantAgent

/// URLProtocol strips `httpBody` — recover via the body stream so tests can
/// assert on POST payloads.
extension URLRequest {
    func bodyData() -> Data {
        if let body = httpBody { return body }
        guard let stream = httpBodyStream else { return Data() }
        var data = Data()
        stream.open()
        defer { stream.close() }
        var buf = [UInt8](repeating: 0, count: 4096)
        while stream.hasBytesAvailable {
            let n = stream.read(&buf, maxLength: buf.count)
            if n <= 0 { break }
            data.append(buf, count: n)
        }
        return data
    }
}

/// Build an `AgentBackend` whose `ActantClient` is wired to MockURLProtocol.
func makeBackend(
    workspaceID: String = "ws_default",
    actorID: String = "act_user",
    token: String? = nil
) -> AgentBackend {
    let client = ActantClient(
        baseURL: URL(string: "http://127.0.0.1:4555")!,
        token: token,
        urlSession: MockURLProtocol.makeSession()
    )
    return AgentBackend(client: client, workspaceID: workspaceID, actorID: actorID)
}
