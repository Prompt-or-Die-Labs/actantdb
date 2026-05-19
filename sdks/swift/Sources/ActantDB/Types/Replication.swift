import Foundation

// MARK: - Replication / sync types
//
// These types describe the *replication-ledger* shape used by the embedded
// (FFI) path and the CloudKit sync target. They are intentionally distinct
// from the HTTP/storage-row shapes (`AgentEvent`, `SyncEvent`, `CommandResponse`):
//
//   - `EventRow` carries the per-event metadata that cross-device sync needs
//     (`device_id`, HLC physical/logical timestamps, full payload bytes).
//   - `HLCCursor` is the per-device sync cursor — a Hybrid Logical Clock
//     value with both wall-clock and logical components.
//   - `CommandOutcome` is the FFI-shape dispatch result returned by
//     `ActantHandle::dispatch` (Rust side, see IOS_EMBEDDING.md §1).
//   - `IngestReport` is what `ActantHandle::ingest` returns after applying a
//     batch of inbound events.
//
// See `docs/SYNC_DESIGN.md` for the CKRecord schema and `docs/IOS_EMBEDDING.md`
// for the FFI surface these mirror.

/// A Hybrid Logical Clock value. `physicalMS` is the wall-clock component in
/// milliseconds since the Unix epoch; `logical` is a per-process counter that
/// increments to break ties when two events share the same physical timestamp.
///
/// `Comparable` orders by `(physicalMS, logical)` lexicographically, matching
/// the SQLite index `idx_agent_event_hlc` defined in migration `0007`.
public struct HLCCursor: Codable, Sendable, Hashable, Comparable {
    public let physicalMS: Int64
    public let logical: UInt32

    public init(physicalMS: Int64, logical: UInt32) {
        self.physicalMS = physicalMS
        self.logical = logical
    }

    public static func < (lhs: HLCCursor, rhs: HLCCursor) -> Bool {
        if lhs.physicalMS != rhs.physicalMS { return lhs.physicalMS < rhs.physicalMS }
        return lhs.logical < rhs.logical
    }

    enum CodingKeys: String, CodingKey {
        case physicalMS = "hlc_physical_ms"
        case logical    = "hlc_logical"
    }
}

/// One replication-ledger event row. This is the unit of replication over
/// CloudKit; one `EventRow` maps to one `CKRecord` of type `ActantEvent`.
///
/// Distinct from `AgentEvent` (storage-row shape returned by `/v1/events`) and
/// `SyncEvent` (slim shape returned by `/v1/sync/since`) — those predate the
/// HLC + `device_id` columns added in migration `0007`.
public struct EventRow: Codable, Sendable, Identifiable, Hashable {
    public let id: String
    public let workspaceID: String
    public let sessionID: String?
    public let deviceID: String
    public let actorID: String
    public let eventType: String
    public let payloadJSON: Data
    public let payloadHash: String
    public let prevChainHash: String?
    public let hlc: HLCCursor
    public let createdAt: String

    public init(
        id: String,
        workspaceID: String,
        sessionID: String? = nil,
        deviceID: String,
        actorID: String,
        eventType: String,
        payloadJSON: Data,
        payloadHash: String,
        prevChainHash: String? = nil,
        hlc: HLCCursor,
        createdAt: String
    ) {
        self.id = id
        self.workspaceID = workspaceID
        self.sessionID = sessionID
        self.deviceID = deviceID
        self.actorID = actorID
        self.eventType = eventType
        self.payloadJSON = payloadJSON
        self.payloadHash = payloadHash
        self.prevChainHash = prevChainHash
        self.hlc = hlc
        self.createdAt = createdAt
    }

    enum CodingKeys: String, CodingKey {
        case id
        case workspaceID    = "workspace_id"
        case sessionID      = "session_id"
        case deviceID       = "device_id"
        case actorID        = "actor_id"
        case eventType      = "event_type"
        case payloadJSON    = "payload_json"
        case payloadHash    = "payload_hash"
        case prevChainHash  = "prev_chain_hash"
        case hlcPhysicalMS  = "hlc_physical_ms"
        case hlcLogical     = "hlc_logical"
        case createdAt      = "created_at"
    }

    public init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.id            = try c.decode(String.self, forKey: .id)
        self.workspaceID   = try c.decode(String.self, forKey: .workspaceID)
        self.sessionID     = try c.decodeIfPresent(String.self, forKey: .sessionID)
        self.deviceID      = try c.decode(String.self, forKey: .deviceID)
        self.actorID       = try c.decode(String.self, forKey: .actorID)
        self.eventType     = try c.decode(String.self, forKey: .eventType)
        // payload_json arrives as a base64-encoded string on the wire to keep
        // it round-trippable through JSON; the in-memory shape is raw Data.
        if let b64 = try c.decodeIfPresent(String.self, forKey: .payloadJSON) {
            guard let data = Data(base64Encoded: b64) else {
                throw DecodingError.dataCorruptedError(
                    forKey: .payloadJSON,
                    in: c,
                    debugDescription: "payload_json is not valid base64"
                )
            }
            self.payloadJSON = data
        } else {
            self.payloadJSON = Data()
        }
        self.payloadHash    = try c.decode(String.self, forKey: .payloadHash)
        self.prevChainHash  = try c.decodeIfPresent(String.self, forKey: .prevChainHash)
        let physical        = try c.decode(Int64.self, forKey: .hlcPhysicalMS)
        let logical         = try c.decode(UInt32.self, forKey: .hlcLogical)
        self.hlc            = HLCCursor(physicalMS: physical, logical: logical)
        self.createdAt      = try c.decode(String.self, forKey: .createdAt)
    }

    public func encode(to encoder: Encoder) throws {
        var c = encoder.container(keyedBy: CodingKeys.self)
        try c.encode(id,                    forKey: .id)
        try c.encode(workspaceID,           forKey: .workspaceID)
        try c.encodeIfPresent(sessionID,    forKey: .sessionID)
        try c.encode(deviceID,              forKey: .deviceID)
        try c.encode(actorID,               forKey: .actorID)
        try c.encode(eventType,             forKey: .eventType)
        try c.encode(payloadJSON.base64EncodedString(), forKey: .payloadJSON)
        try c.encode(payloadHash,           forKey: .payloadHash)
        try c.encodeIfPresent(prevChainHash, forKey: .prevChainHash)
        try c.encode(hlc.physicalMS,        forKey: .hlcPhysicalMS)
        try c.encode(hlc.logical,           forKey: .hlcLogical)
        try c.encode(createdAt,             forKey: .createdAt)
    }
}

/// FFI-shape result of `ActantHandle::dispatch`. Mirrors the wire shape of
/// `CommandResponse` but uses snake-cased coding keys that match the
/// uniffi-generated Rust struct serialization (see IOS_EMBEDDING.md §1).
public struct CommandOutcome: Codable, Sendable, Hashable {
    public let commandID: String
    public let eventID: String?
    public let result: JSONValue

    public init(commandID: String, eventID: String? = nil, result: JSONValue = .null) {
        self.commandID = commandID
        self.eventID = eventID
        self.result = result
    }

    enum CodingKeys: String, CodingKey {
        case commandID = "command_id"
        case eventID   = "event_id"
        case result
    }
}

/// Result of an `ingest(events:)` call. `accepted` rows were inserted;
/// `skipped` rows already existed (same content-derived id) and were
/// idempotently dropped; `rejected` rows failed validation (e.g. hash
/// mismatch) and the caller may want to log them.
public struct IngestReport: Codable, Sendable, Hashable {
    public let accepted: UInt32
    public let skipped: UInt32
    public let rejected: [String]

    public init(accepted: UInt32, skipped: UInt32, rejected: [String] = []) {
        self.accepted = accepted
        self.skipped = skipped
        self.rejected = rejected
    }
}
