import Foundation

#if ACTANTDB_LOCAL_FFI
import ActantFFI

extension ActantHandle: @unchecked Sendable {}

public actor ActantFFIBridge: EmbeddedHandle {
    private let inner: ActantHandle
    private let workspaceID: String
    private let actorID: String

    public init(storeDir: URL, workspaceID: String, actorID: String) async throws {
        self.workspaceID = workspaceID
        self.actorID = actorID
        self.inner = try await ActantHandle.open(
            storeDir: storeDir.path,
            workspaceId: workspaceID,
            actorId: actorID
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
        let ffi = try await inner.dispatch(
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
        let cursorPhysicalMS = try cursor.map { try Self.unsignedHLCPhysicalMS($0.physicalMS) }
        let ffiRows = try await inner.eventsSince(
            cursorHlcPhysicalMs: cursorPhysicalMS,
            cursorHlcLogical: cursor?.logical,
            limit: limit
        )
        return try ffiRows.map { try self.convertEventRow($0) }
    }

    public func ingest(events: [EventRow]) async throws -> IngestReport {
        let ffiEvents = try events.map(Self.convertEventRow)
        let ffi = try await inner.ingest(events: ffiEvents)
        return Self.convertIngestReport(ffi)
    }

    public func close() async {
        await inner.close()
    }

    private static func convertOutcome(_ ffi: ActantFFI.CommandOutcome) throws -> CommandOutcome {
        let result = try JSONDecoder.actant.decode(JSONValue.self, from: Data(ffi.resultJson.utf8))
        return CommandOutcome(
            commandID: ffi.commandId,
            eventID: ffi.eventId,
            result: result
        )
    }

    private func convertEventRow(_ ffi: ActantFFI.EventRow) throws -> EventRow {
        let physicalMS = try Self.signedHLCPhysicalMS(ffi.hlcPhysicalMs)
        return EventRow(
            id: ffi.id,
            workspaceID: workspaceID,
            sessionID: ffi.sessionId,
            deviceID: ffi.deviceId,
            actorID: actorID,
            eventType: ffi.eventType,
            payloadJSON: Data(ffi.payloadJson.utf8),
            payloadHash: ffi.payloadHash,
            prevChainHash: nil,
            hlc: HLCCursor(physicalMS: physicalMS, logical: ffi.hlcLogical),
            createdAt: ffi.createdAt
        )
    }

    private static func convertEventRow(_ row: EventRow) throws -> ActantFFI.EventRow {
        guard let payloadJSON = String(data: row.payloadJSON, encoding: .utf8) else {
            throw ActantError.decoding("EventRow.payloadJSON is not valid UTF-8", body: row.payloadJSON)
        }
        return ActantFFI.EventRow(
            id: row.id,
            sessionId: row.sessionID,
            eventType: row.eventType,
            payloadJson: payloadJSON,
            payloadHash: row.payloadHash,
            createdAt: row.createdAt,
            deviceId: row.deviceID,
            hlcPhysicalMs: try unsignedHLCPhysicalMS(row.hlc.physicalMS),
            hlcLogical: row.hlc.logical
        )
    }

    private static func convertIngestReport(_ ffi: ActantFFI.IngestReport) -> IngestReport {
        IngestReport(accepted: ffi.accepted, skipped: ffi.skipped, rejected: ffi.rejected)
    }

    private static func signedHLCPhysicalMS(_ value: UInt64) throws -> Int64 {
        guard value <= UInt64(Int64.max) else {
            throw ActantError.decoding("hlc_physical_ms exceeds Int64.max", body: Data())
        }
        return Int64(value)
    }

    private static func unsignedHLCPhysicalMS(_ value: Int64) throws -> UInt64 {
        guard value >= 0 else {
            throw ActantError.transport("hlc_physical_ms must be non-negative")
        }
        return UInt64(value)
    }
}

#else

/// Fallback bridge for builds where the `ActantFFI` binary target is not
/// linked (e.g. local `swift build` before the first XCFramework release ships,
/// or non-Apple platforms). Every method throws `ActantError.transport` with a
/// stable message so consumers can branch on it; `Actant.embedded(...)` fails
/// fast at construction time.
public actor ActantFFIBridge: EmbeddedHandle {
    public init(storeDir: URL, workspaceID: String, actorID: String) async throws {
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
