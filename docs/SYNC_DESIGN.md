# SYNC_DESIGN — multi-device agent state sync

Companion to [IOS_EMBEDDING.md](./IOS_EMBEDDING.md). That doc covers
embedding the Rust core; this one covers replicating ledger state across
devices (mobile / home / remote / local) once each device has its own
embedded core.

## Goal

A user's agent state is the same on every device they own. They start a
conversation on the iPhone, walk to the desk, the same conversation
continues on the Mac. They approve a tool call from the watch; the Mac
sees the approval. They go offline on the train; everything queues up;
when they come back online, every device converges.

No infra to run. No account to create. No third party reading their
chats.

## Transport: CloudKit private database

The meeting point is **CloudKit private database**. Why:

- **Zero infra.** Apple runs the storage; no cloud control plane to ship
  for the v1 sync story.
- **End-to-end encryptable.** CloudKit encrypts in transit + at rest;
  Apple sees opaque blobs.
- **Content-blind to Apple.** Sync data is just CKRecords; Apple has no
  visibility into the ledger contents.
- **Free up to consumer storage quotas.** Apple gives every iCloud
  account 5 GB free; even chatty agent state is small relative to that.
- **Push notification for resume.** CloudKit subscriptions wake a sleeping
  device when a new record lands — no polling needed.
- **Container-scoped.** One iCloud container per app; isolated from other
  apps the user runs.

What CloudKit gives up:

- **Apple-ecosystem only.** Android / Linux / Windows devices can't be
  the meeting point. For cross-OS sync we'd need a real ActantDB Cloud
  relay (CLOUD_GAPS.md row G1, post-Phase-2). CloudKit is the right
  Phase-1 answer because the immediate target (Swoosh + the user's own
  fleet) is all-Apple.
- **No fan-out beyond the iCloud account.** Sync between two *people*
  needs a different shape (workspace-shared or hosted relay).

## Replication shape

### What flows over the wire

The ledger is append-only. The sync unit is **one event row per CKRecord**:

```
CKRecord(recordType: "ActantEvent")
  - id: String              # event id (content-derived hash)
  - workspace_id: String
  - session_id: String?
  - device_id: String       # who wrote it
  - actor_id: String
  - event_type: String
  - payload_json: Bytes     # the same JSON the ledger stores
  - payload_hash: String
  - prev_chain_hash: String?
  - hlc_physical_ms: Int64
  - hlc_logical: Int64
  - created_at: String      # RFC 3339
```

Each device subscribes to its workspace's record zone. CloudKit pushes
new records; the receiver calls `ActantHandle::ingest(events_ndjson)`.

### Cursor + delta

Each device persists a per-workspace `sync_cursor` = the last
`(hlc_physical_ms, hlc_logical)` it has acknowledged. On wake:

1. Query CloudKit for records `WHERE workspace_id == $ws AND (hlc > $cursor)`.
2. Stream the rows through `events_since` decoder.
3. Call `ingest(batch)` on the local ActantHandle.
4. Advance cursor.

Push side: every local write goes through the existing `actant-storage`
append path AND a new `cloudkit_outbox` deferred-publish queue. A background
task drains the outbox, creates CKRecords, retries on failure (the
`actant-reliability::circuit` substrate covers this).

### Idempotency + conflict freedom

Events are append-only with content-derived IDs (see
[IOS_EMBEDDING.md](./IOS_EMBEDDING.md) §4). Same event ingested twice ->
INSERT OR IGNORE on the PK. Two devices appending concurrent events
both win — HLC orders them deterministically; the ledger holds both;
neither overwrites the other.

The only place conflict resolution matters is **projections** (memory
approval state, session phase, actor display name). Per-field LWW by
HLC is the default (`crates/actant-replay/src/conflict.rs`). Documented
per-record-type in IOS_EMBEDDING.md §5.

## Failure modes

| Failure | What happens |
|---|---|
| Device offline | Local ledger keeps growing; outbox queue grows. On reconnect, drains. |
| CloudKit throttling | Outbox retries with exponential backoff via `actant-reliability::circuit`. |
| User signs out of iCloud | Local ledger stays; sync stops; on next sign-in, full replay from CloudKit (capped at 30 days by default — older state stays only on the original device). |
| CloudKit corruption / bug | Local ledger is canonical. CloudKit is sync only; rebuilding the CKRecord set from local is one `for event in ledger.query_all() { ckRecord.save(event) }` loop. |
| Two devices write to the same projection field simultaneously | HLC-LWW picks the winner; the loser's value is in the chain (audit-recoverable) but doesn't show in the projection. |
| Clock skew between devices | HLC handles skew up to a few minutes; beyond that, devices reject incoming events with `hlc_physical_ms - local_physical_ms > 5min` and emit a `clock_skew_detected` event. Surfaces in Studio. |
| Large attachment doesn't fit in CKRecord (1 MB limit) | Attachment goes to `CKAsset` (CloudKit's S3 equivalent); the event row holds the `asset_id`. Already matches `actant-objectstore`'s `BlobRef` shape. |

## API on the Swift side

```swift
import ActantDB

let actant = try await Actant.embedded(
    storeDir: appSupportDir,
    workspaceID: "ws_default"
)

// Opt into CloudKit sync (no-op on non-Apple platforms once we add them):
try await actant.sync.enable(
    container: "iCloud.com.swoosh.actant",
    options: .init(
        retainDays: 30,       // CloudKit-side TTL
        pushOnAppActive: true // resume push subscriptions on appWillEnterForeground
    )
)

// At any point:
let state = try await actant.sync.status()
// .syncedAtHLC, .outboxQueueDepth, .lastInboundEventAt, .activeSubscription
```

iOS-specific Swift-side code lives in `Sources/ActantDB/Sync/CloudKit/`.
Other platforms get `Sources/ActantDB/Sync/None/` (no-op stubs that report
`SyncError.unsupportedPlatform`). Future Android sync would land
`Sources/ActantDB/Sync/Drive/` etc.

## What we do NOT ship (today)

- **A hosted ActantDB Cloud sync relay** — that's Phase 2/3 work in
  CLOUD_GAPS.md. CloudKit is good enough for the first cohort of users
  who live in the Apple ecosystem.
- **CRDTs for arbitrary state.** Event-sourced append-only + projection
  LWW is the simplest correct thing. We add CRDT-typed columns only when
  a consumer's specific projection needs it (e.g. a counter that should
  increment from multiple devices concurrently).
- **WebRTC / Wi-Fi Direct local-only mode.** Two devices on the same LAN
  could sync directly without going to CloudKit. Possible later; not now.

## Substrate work (cross-link to GAPS)

| New rows (GAPS.md) | Status |
|---|---|
| Stable content-derived event IDs (`actant-core`) | 🔴 — depends on HLC |
| HLC clock implementation (`actant-core`) | 🔴 |
| `device_id` column on `agent_event` (migration `0007`) | 🔴 |
| `ingest(events)` idempotent API on `Storage` | 🔴 |
| Conflict policy table (`actant-replay/src/conflict.rs`) | 🔴 |
| `cloudkit_outbox` table (per-device persistent queue) | 🔴 |
| Swift `actant.sync.enable(container:)` API | 🔴 |
| `Sources/ActantDB/Sync/CloudKit/` impl | 🔴 |

| New rows (CLOUD_GAPS.md) | Status |
|---|---|
| ActantDB Cloud sync relay (cross-OS) | 🌐 Phase 3 |
| Group / workspace-shared sync (multi-tenant) | 🌐 Phase 3 |

## Open questions to resolve before implementation starts

1. **iCloud container naming** — `iCloud.com.actantdb.shared`? Each
   consumer app's own container? Decision affects sharing model.
2. **Encryption** — CKRecord fields are CloudKit-encrypted by default;
   should `payload_json` also be app-encrypted (so even Apple-attested
   secrets stay opaque)? Probably yes for `secret`-sensitivity payloads;
   consumer can opt-in for all.
3. **Retain policy** — 30-day cloud TTL is the default; should we offer
   "keep forever"? Storage cost lands on the user's iCloud quota; needs
   explicit opt-in.
4. **Multi-account devices** — what if a single Mac is signed into two
   iCloud accounts? CloudKit handles this; we follow whichever account
   the app is launched under. Document but don't try to be clever.

These get answered the first time a consumer pushes back on the default;
defer until then.
