import Foundation
import ActantDB

/// Tiny ledger-backed audit log. `log(record)` writes a JSON-sentinel-wrapped
/// payload as an agent message in the session; `last()` walks the session's
/// events in reverse and returns the most recent sentinel-bearing record.
///
/// Wire shape of the log message text:
///
///     {"<sentinelKey>": {"v":1, "r": <encoded Record>}}
///
/// This keeps the wrapper schema-stable while letting `Record` evolve.
public struct Auditor<Record: Codable & Sendable>: Sendable {
    public let backend: AgentBackend
    public let sessionID: String
    public let sentinelKey: String

    public init(backend: AgentBackend, sessionID: String, sentinelKey: String) {
        self.backend = backend
        self.sessionID = sessionID
        self.sentinelKey = sentinelKey
    }

    public func log(_ record: Record) async throws {
        let client      = backend.client
        let workspaceID = backend.workspaceID
        let actorID     = backend.actorID

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        let recordData: Data
        do {
            recordData = try encoder.encode(record)
        } catch {
            throw ActantError.transport("Auditor.log: encode failed: \(error)")
        }
        // Wrap as JSONValue so the outer JSON shape is uniform.
        let recordValue: JSONValue
        do {
            recordValue = try JSONDecoder.actant.decode(JSONValue.self, from: recordData)
        } catch {
            throw ActantError.transport("Auditor.log: re-decode failed: \(error)")
        }
        let envelope: JSONValue = .object([
            sentinelKey: .object([
                "v": .int(1),
                "r": recordValue,
            ]),
        ])
        let envelopeData = try encoder.encode(envelope)
        guard let text = String(data: envelopeData, encoding: .utf8) else {
            throw ActantError.transport("Auditor.log: envelope is not UTF-8")
        }

        _ = try await client.appendAgentMessage(
            workspaceID: workspaceID, actorID: actorID,
            sessionID: sessionID, text: text
        )
    }

    public func last() async throws -> Record? {
        let client = backend.client
        let events = try await client.events(sessionID: sessionID)

        let decoder = JSONDecoder()

        // Walk newest-first; first sentinel match wins.
        for event in events.reversed() {
            guard let payload = try? event.parsedPayload() else { continue }

            // Payload schema is `{"text":"<json string>"}` for message events.
            // Extract `text`, parse as JSON, look for sentinelKey.
            guard case let .object(outer) = payload,
                  case let .string(text)? = outer["text"],
                  let data = text.data(using: .utf8) else { continue }

            guard let envelope = try? decoder.decode(JSONValue.self, from: data),
                  case let .object(envObj) = envelope,
                  case let .object(inner)? = envObj[sentinelKey],
                  let recordValue = inner["r"]
            else { continue }

            let recordData = try JSONEncoder.actant.encode(recordValue)
            do {
                return try decoder.decode(Record.self, from: recordData)
            } catch {
                throw ActantError.transport("Auditor.last: decode failed: \(error)")
            }
        }
        return nil
    }
}

// JSONEncoder/JSONDecoder extensions live in ActantDB; we use them via
// `ActantDB`'s public extensions on Foundation types.
private extension JSONEncoder {
    static var actant: JSONEncoder {
        let e = JSONEncoder()
        e.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return e
    }
}

private extension JSONDecoder {
    static var actant: JSONDecoder { JSONDecoder() }
}
