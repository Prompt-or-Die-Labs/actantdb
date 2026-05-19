import Foundation
import ActantDB

/// Conversational role a message belongs to. Mirrors typical chat semantics
/// (`user` / `assistant` / `tool` / `system`) — the underlying ledger only
/// has `append_user_message` / `append_agent_message`, so `tool` / `system`
/// fall back to agent-message with a `[role=<role>]` prefix so they survive
/// a round-trip.
///
/// Note: this is `.assistant`, not `.agent`. The ledger event type is
/// `agent_message` but the conversational role we expose follows the
/// industry-standard chat-completion vocabulary (matches OpenAI / Anthropic
/// / Vertex / etc.) so consumer `Codable` types map cleanly without a
/// translation layer.
public enum SessionRole: String, Sendable, Codable {
    case user
    case assistant
    case tool
    case system
}

/// Append-and-load wrapper over the `agent_event` table for a single
/// session. Generic over the consumer's own message type so the SDK never
/// needs to know about, e.g., `SwooshCore.ChatMessage`.
///
/// The consumer supplies two closures:
///   - `encode(message)` → `(role, text)` to write
///   - `decode(role, text, createdAt)` → optional `Message` (return `nil`
///     to skip a row, e.g., a malformed legacy event)
public struct Session<Message: Sendable>: Sendable {
    public let backend: AgentBackend
    public let sessionID: String

    public let encode: @Sendable (Message) -> (role: SessionRole, text: String)
    public let decode: @Sendable (SessionRole, String, Date) -> Message?

    public init(
        backend: AgentBackend,
        sessionID: String,
        encode: @Sendable @escaping (Message) -> (role: SessionRole, text: String),
        decode: @Sendable @escaping (SessionRole, String, Date) -> Message?
    ) {
        self.backend = backend
        self.sessionID = sessionID
        self.encode = encode
        self.decode = decode
    }

    /// Encode `message` via the supplied `encode` closure and dispatch the
    /// matching ledger command. `tool` and `system` roles fall back to
    /// `append_agent_message` with a `[role=<role>]` text prefix so they
    /// can be distinguished on load.
    public func appendMessage(_ message: Message) async throws {
        let (role, text) = encode(message)
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let actorID     = backend.actorID

        switch role {
        case .user:
            _ = try await client.appendUserMessage(
                workspaceID: workspaceID, actorID: actorID,
                sessionID: sessionID, text: text
            )
        case .assistant:
            _ = try await client.appendAgentMessage(
                workspaceID: workspaceID, actorID: actorID,
                sessionID: sessionID, text: text
            )
        case .tool, .system:
            let prefixed = "[role=\(role.rawValue)] \(text)"
            _ = try await client.appendAgentMessage(
                workspaceID: workspaceID, actorID: actorID,
                sessionID: sessionID, text: prefixed
            )
        }
    }

    /// Load the session's transcript in ledger order. Walks every
    /// `user_message` / `agent_message` event, extracts the `text` field
    /// from the inline payload, infers role, and hands it to `decode`.
    /// Rows the consumer returns `nil` for are skipped.
    public func loadTranscript() async throws -> [Message] {
        let client = backend.client
        let events = try await client.events(sessionID: sessionID)

        var out: [Message] = []
        out.reserveCapacity(events.count)

        for event in events {
            let role: SessionRole
            switch event.eventType {
            case "user_message", "user_message_received":
                role = .user
            case "agent_message", "agent_message_sent":
                role = .assistant
            default:
                continue
            }

            guard let payload = try? event.parsedPayload(),
                  case let .object(obj) = payload,
                  case let .string(text)? = obj["text"]
            else { continue }

            let date = Self.parseDate(event.createdAt) ?? Date()

            // Promote the role for tool/system messages that were stored
            // via the `[role=<role>] ` prefix shim in `appendMessage`.
            let (effectiveRole, effectiveText): (SessionRole, String)
            if role == .assistant, text.hasPrefix("[role=") {
                if let closing = text.firstIndex(of: "]") {
                    let tagBody = text[text.index(text.startIndex, offsetBy: 6)..<closing]
                    let rest    = text[text.index(after: closing)...]
                        .drop(while: { $0 == " " })
                    if let promoted = SessionRole(rawValue: String(tagBody)) {
                        effectiveRole = promoted
                        effectiveText = String(rest)
                    } else {
                        effectiveRole = role
                        effectiveText = text
                    }
                } else {
                    effectiveRole = role
                    effectiveText = text
                }
            } else {
                effectiveRole = role
                effectiveText = text
            }

            if let m = decode(effectiveRole, effectiveText, date) {
                out.append(m)
            }
        }

        return out
    }

    /// ISO-8601 with fractional seconds. Falls back to plain ISO-8601.
    static func parseDate(_ s: String) -> Date? {
        let f1 = ISO8601DateFormatter()
        f1.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let d = f1.date(from: s) { return d }
        let f2 = ISO8601DateFormatter()
        f2.formatOptions = [.withInternetDateTime]
        return f2.date(from: s)
    }
}
