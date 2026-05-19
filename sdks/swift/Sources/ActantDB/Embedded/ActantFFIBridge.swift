import Foundation

// MARK: - ActantFFI bridge
//
// This file is the consumer-side seam over the uniffi-generated
// `ActantHandle` Swift class produced by `crates/actant-ffi`. The generated
// glue is shipped as the `ActantFFI` binary target (see `.binaryTarget` in
// `Package.swift`); when that target is missing, the bridge falls back to a
// stub that throws on every method so the package still compiles.
//
// Expected generated surface (per IOS_EMBEDDING.md §1, finalized by the
// parallel agent building `crates/actant-ffi`):
//
//     import ActantFFI
//
//     public class ActantHandle {
//         public static func open(storeDir: String, workspaceId: String)
//             throws -> ActantHandle
//         public func dispatch(
//             commandType: String,
//             inputJson: String,
//             idempotencyKey: String?
//         ) throws -> ActantFFICommandOutcome
//         public func eventsSince(cursor: String?, limit: UInt32)
//             throws -> [ActantFFIEventRow]
//         public func ingest(eventsNdjson: String) throws -> ActantFFIIngestReport
//         public func close()
//     }
//
// TODO(ffi): once `crates/actant-ffi` lands, drop the stub branch, switch the
// production branch to call the generated `ActantHandle`, and convert the
// generated FFI types to the SDK-facing `CommandOutcome` / `EventRow` /
// `IngestReport` types.

#if canImport(ActantFFI)
import ActantFFI

/// Production bridge backed by the uniffi-generated `ActantHandle`. The
/// generated class is reference-typed and thread-safe per uniffi's runtime
/// guarantees; we wrap it in an actor so the SDK's async surface composes
/// without requiring callers to reason about FFI threading.
public actor ActantFFIBridge: EmbeddedHandle {
    private let inner: ActantHandle

    public init(storeDir: URL, workspaceID: String, actorID: String) throws {
        // `actorID` is currently informational on the FFI side; the Rust core
        // reads the configured workspace + per-event actor at dispatch time.
        _ = actorID
        self.inner = try ActantHandle.open(
            storeDir: storeDir.path,
            workspaceId: workspaceID
        )
    }

    public func dispatch(
        commandType: String,
        input: JSONValue,
        idempotencyKey: String?
    ) async throws -> CommandOutcome {
        let inputJSON = try String(
            data: JSONEncoder.actant.encode(input),
            encoding: .utf8
        ) ?? "null"
        let ffi = try inner.dispatch(
            commandType: commandType,
            inputJson: inputJSON,
            idempotencyKey: idempotencyKey
        )
        return try Self.convertOutcome(ffi)
    }

    public func eventsSince(
        cursor: HLCCursor?,
        limit: UInt32
    ) async throws -> [EventRow] {
        let cursorString = cursor.map { "\($0.physicalMS):\($0.logical)" }
        let ffiRows = try inner.eventsSince(cursor: cursorString, limit: limit)
        return try ffiRows.map(Self.convertEventRow)
    }

    public func ingest(events: [EventRow]) async throws -> IngestReport {
        let ndjson = try events.map { row -> String in
            let data = try JSONEncoder.actant.encode(row)
            return String(data: data, encoding: .utf8) ?? ""
        }.joined(separator: "\n")
        let ffi = try inner.ingest(eventsNdjson: ndjson)
        return Self.convertIngestReport(ffi)
    }

    public func close() async {
        inner.close()
    }

    // MARK: - Conversion helpers (FFI ↔ SDK types)
    //
    // !!! PLACEHOLDER — WILL NOT COMPILE AGAINST REAL `ActantFFI` MODULE !!!
    //
    // The bodies below treat the uniffi-generated types as `Any` and try a
    // JSON round-trip, which fails at compile time the moment a real
    // `ActantFFI` module exposes the typed records (uniffi-generated structs
    // are not `Encodable`). This is intentional: the bridge has to be
    // rewritten once the parallel `crates/actant-ffi` agent finalizes the
    // generated Swift surface (see GAPS row #39). At that point:
    //   1. Replace `_ ffi: Any` with the concrete generated record types
    //      (e.g. `ActantFFICommandOutcome`).
    //   2. Map field-by-field — uniffi gives camelCased property accessors.
    //   3. Drop the JSON round-trip entirely.
    //
    // Keeping the bridge file present (rather than stubbing it out entirely)
    // means consumers see the planned shape today; the integration agent
    // touches one file when the FFI surface is real.

    private static func convertOutcome(_ ffi: Any) throws -> CommandOutcome {
        if let data = try? JSONEncoder().encode(AnyEncodable(ffi)),
           let decoded = try? JSONDecoder.actant.decode(CommandOutcome.self, from: data) {
            return decoded
        }
        return CommandOutcome(commandID: "ffi_unknown", eventID: nil, result: .null)
    }

    private static func convertEventRow(_ ffi: Any) throws -> EventRow {
        let data = try JSONEncoder().encode(AnyEncodable(ffi))
        return try JSONDecoder.actant.decode(EventRow.self, from: data)
    }

    private static func convertIngestReport(_ ffi: Any) -> IngestReport {
        if let data = try? JSONEncoder().encode(AnyEncodable(ffi)),
           let decoded = try? JSONDecoder.actant.decode(IngestReport.self, from: data) {
            return decoded
        }
        return IngestReport(accepted: 0, skipped: 0, rejected: [])
    }
}

#else

/// Fallback bridge for builds where the `ActantFFI` binary target is not
/// linked (e.g. local `swift build` before the first XCFramework release ships,
/// or non-Apple platforms). Every method throws `ActantError.transport` with a
/// stable message so consumers can branch on it; `Actant.embedded(...)` fails
/// fast at construction time.
public actor ActantFFIBridge: EmbeddedHandle {
    public init(storeDir: URL, workspaceID: String, actorID: String) throws {
        _ = (storeDir, workspaceID, actorID)
        throw ActantError.transport(
            "ActantFFI binary target not linked — see docs/IOS_EMBEDDING.md"
        )
    }

    public func dispatch(
        commandType: String,
        input: JSONValue,
        idempotencyKey: String?
    ) async throws -> CommandOutcome {
        throw ActantError.transport("ActantFFI unavailable")
    }

    public func eventsSince(
        cursor: HLCCursor?,
        limit: UInt32
    ) async throws -> [EventRow] {
        throw ActantError.transport("ActantFFI unavailable")
    }

    public func ingest(events: [EventRow]) async throws -> IngestReport {
        throw ActantError.transport("ActantFFI unavailable")
    }

    public func close() async {}
}

#endif
