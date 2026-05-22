# IOS_EMBEDDING — embedding the ActantDB Rust core on iOS

Per the cross-device sync direction: ActantDB on iOS is *not* a remote-server
client and *not* a subprocess. iOS apps embed the Rust core as a static
library + ride CloudKit private database for sync. This file is the design.

Cross-reference:
- [SYNC_DESIGN.md](./SYNC_DESIGN.md) — replication shape on top of this.
- [GAPS.md](../GAPS.md) — substrate-side row for the FFI work.
- [CLOUD_GAPS.md](../CLOUD_GAPS.md) — Cloud rows for any hosted-side relay.

## Goal

A consumer (Swoosh, any future iOS app using ActantDB) writes:

```swift
import ActantDB

// On iOS: embedded, in-process. No HTTP loopback, no subprocess.
let actant = try Actant.embedded(
    storeDir: FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!,
    workspaceID: "ws_default"
)

// On macOS during development you can still pick:
//   - .remote(url:) — current ActantClient HTTP/WS path
//   - .spawned(supervisor: ActantDBSupervisor()) — current child-process path
//   - .embedded(storeDir:) — same as iOS; useful for tests
```

`Actant` is the unified facade; the three modes are construction-time choices
that produce the same opaque type. Every API works identically.

## Substrate work — six items the Rust side has to ship

These match the user's directive verbatim:

### 1. FFI surface via `uniffi-rs`

New crate at `crates/actant-ffi/`:

```rust
// crates/actant-ffi/src/lib.rs
uniffi::setup_scaffolding!();

#[derive(uniffi::Object)]
pub struct ActantHandle { /* … */ }

#[uniffi::export]
impl ActantHandle {
    #[uniffi::constructor]
    pub fn open(store_dir: String, workspace_id: String) -> Result<Arc<Self>, ActantError> { /* … */ }

    pub fn dispatch(
        &self,
        command_type: String,
        input_json: String,
        idempotency_key: Option<String>,
    ) -> Result<CommandOutcome, ActantError> { /* … */ }

    pub fn events_since(&self, cursor: Option<String>, limit: u32) -> Vec<EventRow> { /* … */ }
    pub fn ingest(&self, events_ndjson: String) -> Result<IngestReport, ActantError> { /* … */ }
    pub fn close(&self) { /* … */ }
}
```

Why uniffi over swift-bridge / hand-rolled C-ABI:

- **uniffi-rs** (Mozilla; powers Firefox iOS, Stripe) — mature, auto-generates
  Swift glue, handles `async`/`Result`/enums/records out of the box.
- **swift-bridge** — more idiomatic Swift types but younger; less mature error
  handling, no async story today.
- **Hand-rolled C-ABI** — fastest to ship the first 100 LOC; biggest regret
  by month six. Manual `unsafe` for every type plus duplicate type
  definitions on each side.

Decision: **uniffi-rs**.

`uniffi-bindgen-swift` generates `ActantHandle.swift` + `ActantHandleFFI.h`
at build time; the Swift side imports + uses them as if they were native.

### 2. iOS-clean Rust core audit

Anything in the workspace that wouldn't run inside the iOS sandbox is a
blocker. The audit table:

| Concern | Where to check | Status |
|---|---|---|
| `std::process::Command` / `tokio::process` (no `posix_spawn` to arbitrary bins) | `crates/actant-workers/src/shell.rs`, `crates/actant-cli/src/main.rs` | iOS-only build skips the worker bin invocations; `actant-cli` is host-only |
| Writes to `~/.actantdb` or anywhere outside the caller-supplied path | every `Storage::open(path)` call site | already takes a path arg — verify no fallback to `home_dir()` exists |
| `libsqlite3` system-linked (iOS sandbox restricts dlopen) | `Cargo.toml` features for `sqlx` | switch the iOS feature profile to `sqlx` with the `sqlite-bundled` flavor (compiles SQLite from source) |
| `reqwest` with `native-tls` (uses Security.framework that's allowed but heavy) | every `reqwest::Client` builder | switch iOS feature profile to `rustls-tls` only — no Security.framework dep |
| `fork`/`exec` in the `actant-reliability::ingress` retry loop | `crates/actant-reliability/src/ingress.rs` | check; if present, replace with `tokio::spawn` |
| `getpid` / `getuid` / process-table reads | `actant-server` binding logic | wrap with `#[cfg(not(target_os = "ios"))]` |
| Filesystem temp dir assumptions | all tests using `tempfile::tempdir()` | already takes a path; iOS gets the app sandbox tempdir via the caller |

The audit ships as `cargo run -p actant-ffi --bin ios-audit` — a script
that greps every workspace member for these concerns and prints a punch list.

### 3. XCFramework build + Package.swift binary target

New CI workflow `.github/workflows/ios-xcframework.yml`:

```yaml
name: ios-xcframework
on:
  push:
    tags: ['v*']
  workflow_dispatch:

jobs:
  build:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.88.0
      - name: Add Apple targets
        run: |
          rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
          rustup target add aarch64-apple-darwin x86_64-apple-darwin
      - name: Build libactant_ffi.a for every target
        run: |
          for target in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin x86_64-apple-darwin; do
            cargo build --release --target "$target" -p actant-ffi
          done
      - name: lipo sim slices
        run: |
          lipo -create \
            target/aarch64-apple-ios-sim/release/libactant_ffi.a \
            target/x86_64-apple-ios/release/libactant_ffi.a \
            -output target/lib-sim.a
          lipo -create \
            target/aarch64-apple-darwin/release/libactant_ffi.a \
            target/x86_64-apple-darwin/release/libactant_ffi.a \
            -output target/lib-mac.a
      - name: xcodebuild -create-xcframework
        run: |
          xcodebuild -create-xcframework \
            -library target/aarch64-apple-ios/release/libactant_ffi.a -headers crates/actant-ffi/include \
            -library target/lib-sim.a -headers crates/actant-ffi/include \
            -library target/lib-mac.a -headers crates/actant-ffi/include \
            -output ActantFFI.xcframework
      - name: Zip + checksum
        run: |
          ditto -c -k --keepParent ActantFFI.xcframework ActantFFI.xcframework.zip
          swift package compute-checksum ActantFFI.xcframework.zip > ActantFFI.checksum
      - name: Upload release artifact
        uses: softprops/action-gh-release@v2
        with:
          files: |
            ActantFFI.xcframework.zip
            ActantFFI.checksum
```

Before the first tagged XCFramework exists, local Swift validation uses the
development hook in `sdks/swift/Package.swift`:

```bash
bash sdks/swift/scripts/build-local-actantffi-xcframework.sh
ACTANTDB_LOCAL_FFI_XCFRAMEWORK=".actantffi/ActantFFI.xcframework" \
  swift test --package-path sdks/swift --filter embeddedRoundTrip
```

That env var only adds local generated Swift + `.binaryTarget(path:)` targets
for the current SwiftPM invocation. The published package manifest remains clean
until the release asset URL and checksum are known.

`sdks/swift/Package.swift` gains a `binaryTarget`:

```swift
.binaryTarget(
    name: "ActantFFI",
    url: "https://github.com/Prompt-or-Die-Labs/actantdb/releases/download/v0.0.X/ActantFFI.xcframework.zip",
    checksum: "<filled from ActantFFI.checksum>"
)
```

The package manifest now models the actual UniFFI shape:

- `actant_ffiFFI` is the binary target carrying the XCFramework C module.
- `ActantFFI` is a Swift source target at `sdks/swift/Sources/ActantFFI/`
  containing the generated `actant_ffi.swift` glue.
- `ActantDB` depends on `ActantFFI` only when a local or released binary
  target is configured, and defines `ACTANTDB_FFI` for that build.

`releasedActantFFI` in `sdks/swift/Package.swift` stays `nil` until the first
release asset URL and SwiftPM checksum exist. Local validation continues to use:

```bash
bash sdks/swift/scripts/build-local-actantffi-xcframework.sh
ACTANTDB_LOCAL_FFI_XCFRAMEWORK=".actantffi/ActantFFI.xcframework" \
  swift test --package-path sdks/swift --filter embeddedRoundTrip
```

### 4. Replication-friendly event semantics

The event ledger already has most of this. Concrete deltas:

| Property | Today | Target |
|---|---|---|
| Stable, content-derived event IDs | ULID monotonic (per-process) | **HLC + sha256(payload, hlc, actor_id)** — same payload from the same actor at the same logical time always hashes the same. Idempotent ingest is then "skip if id already present". |
| Per-event `actor_id` | yes | unchanged |
| Per-event `device_id` | no | **add column**; iOS gets `UIDevice.current.identifierForVendor`, Mac gets a generated UUID stored in `~/Library/Application Support`. Persisted; survives reinstalls per Apple ToS. |
| Lamport / HLC clock | no | **add HLC** (Hybrid Logical Clock — wall clock + counter). 8 bytes; survives process restart by reading the max value in the ledger on open. |
| `events_since(cursor)` | yes (`storage.events_in_session`) | extend to cursor-paginated, all-sessions read with optional `device_id != X` filter for sync. |
| `ingest(events[])` | no | **new method**. Validates each event's id (recompute hash; compare), inserts on conflict-do-nothing on the primary key. |

Schema add (new migration `0007_replication.sql`):

```sql
ALTER TABLE agent_event ADD COLUMN device_id TEXT NOT NULL DEFAULT '_legacy_';
ALTER TABLE agent_event ADD COLUMN hlc_physical_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE agent_event ADD COLUMN hlc_logical INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_agent_event_hlc ON agent_event(hlc_physical_ms, hlc_logical);
CREATE INDEX IF NOT EXISTS idx_agent_event_device ON agent_event(device_id);
```

Mirror in `migrations/pg/0007_replication.sql` to keep parity gate green.

### 5. Conflict policy, documented per record type

Append-only event rows are merge-free by definition. **Projections** —
"is this memory approved?" "what's the latest message preview?" — need a
rule. The default we ship:

- **Per-projection-row last-writer-wins (LWW) by HLC.** Per-field LWW for
  `memory.approved_at`, `session.title`, `actor.display_name`. Everything
  else: row-level LWW.

Lives in `crates/actant-replay/src/conflict.rs` as a small policy table:

```rust
pub struct ConflictPolicy {
    pub per_field_lww: HashMap<&'static str, Vec<&'static str>>,
}

impl ConflictPolicy {
    pub fn default() -> Self {
        Self {
            per_field_lww: [
                ("memory", vec!["approved_at", "rejected_at", "last_verified_at"]),
                ("session", vec!["title", "phase"]),
                ("actor", vec!["display_name"]),
            ].into_iter().collect(),
        }
    }
}
```

Consumer-overridable; documented in the Swift SDK README as part of the
embedded-mode quickstart.

### 6. ActantDBSupervisor gated on iOS

Done in this commit:
`sdks/swift/Sources/ActantAgent/ActantDBSupervisor.swift` is now wrapped
in `#if !os(iOS) ... #endif`. iOS-mode tests in `SupervisorTests.swift`
get the same gate. Unblocks `xcodebuild -scheme ActantAgent -destination
'platform=iOS Simulator,name=iPhone 16'`.

## Execution order

1. **This commit** — supervisor iOS gate (item 6 — done).
2. **Next PR** — `crates/actant-ffi/` skeleton (item 1) with the minimal `open`/`dispatch`/`events_since`/`ingest` surface + uniffi setup.
3. **PR after that** — iOS-clean audit + the feature-flag fixes (item 2).
4. **PR after that** — migration 0007 + HLC implementation in `actant-core` (item 4).
5. **PR after that** — conflict policy + replay/merge surface (item 5).
6. **Release operation** — run the XCFramework workflow, pin `releasedActantFFI`
   in `Package.swift` to the release asset URL plus `ActantFFI.checksum`, then
   tag that commit. Local validation is unblocked via
   `ACTANTDB_LOCAL_FFI_XCFRAMEWORK`.
7. **Cross-link** — `SYNC_DESIGN.md` covers what rides on top.

## Out of scope

- Android equivalent — different sandbox model, JNI vs uniffi binding.
  Tracked under `DEVX_GAPS.md` X31 (Kotlin SDK) + a future Android-FFI doc.
- macOS Catalyst — works for free under the macOS targets, no extra build
  steps. Use the `.macOS(.v26)` slice.
- watchOS / tvOS / visionOS — same FFI works; new XCFramework slices when
  a consumer asks.
