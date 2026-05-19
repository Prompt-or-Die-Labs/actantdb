# Storage Layer Audit

Scope: persistence in ActantDB — SQLite + Postgres backends, object/artifact storage, IPFS / content addressing, backup/restore, and concurrency / replication. Conducted 2026-05-18 against the tree at `/Users/home/actantDB/`.

Confidence is per gap. Production blockers are flagged at the top of each section.

---

## 1. Headline

**The Helm chart's `storage.backend=postgres` mode and the docker-compose `ACTANTDB_DATABASE_URL=postgres://...` are silently ignored.** `actant-server::bootstrap()` (`crates/actant-server/src/lib.rs:2011-2025`) never reads any URL env var and never constructs `PgStorage`. It unconditionally opens `actant_storage::Storage` (SQLite) and proceeds. The deployment looks healthy; the wrong backend is running. This is a worse failure mode than `NotImplemented` — there is no error.

Below the server, the picture is consistent: `PgStorage` exists as a connection wrapper with a 7-table schema, but no repo methods, and the rest of the substrate takes `&Storage` directly (the SQLite type), not the `StorageBackend` enum. The PG branch is plumbing without a building behind it.

---

## 2. Backend completeness — SQLite vs Postgres

### What's in the crate

- `crates/actant-storage/src/lib.rs` — `Storage` (SQLite handle, pool, migration runner). 197 lines.
- `crates/actant-storage/src/postgres.rs` — `PgStorage` (Postgres handle, pool, migration runner). 127 lines.
- `crates/actant-storage/src/backend.rs` — `StorageBackend` enum (Sqlite | Postgres) + `PG_NOT_IMPLEMENTED_HINT`. 109 lines.
- `crates/actant-storage/src/repo.rs` — typed repo methods. **Only `impl Storage`. Zero `impl PgStorage`.** 330 lines.

### Schema parity

| Backend  | Migration files                                                            | `CREATE TABLE` count |
| -------- | -------------------------------------------------------------------------- | -------------------- |
| SQLite   | `migrations/0001_initial.sql` + `0002_extended_primitives.sql` + `0003_ai_native_and_reliability.sql` | 41 + 16 + 30 = **87** |
| Postgres | `migrations/pg/0001_initial.sql`                                            | **7**                |

The 7 tables the PG migration ships: `workspace`, `actor`, `session`, `message`, `agent_event`, `command_record`, `idempotency_record` (`migrations/pg/0001_initial.sql:15-107`). Even the PG migration's own header (line 5) admits "Full parity with `/specs/02-data-model.sql` will land in 6.5."

Tables present in SQLite but **absent in Postgres** (incomplete list, just the ones that block real workloads):

- `tool`, `tool_schema_version`, `tool_call` — no tool calls can be recorded.
- `approval_request`, `effect`, `effect_result`, `effect_claim`, `worker`, `worker_capability`, `worker_heartbeat` — no effects / workers / approvals.
- `memory_candidate`, `memory`, `memory_use` — no memory subsystem.
- `workflow`, `workflow_node`, `workflow_edge`, `workflow_run`, `workflow_step_run`, `trigger`, `agent_task` — no workflows.
- `artifact`, `audit_event`, `replay_checkpoint`, `replay_run`, `replay_diff` — no artifacts, no audit projection, no replay.
- `policy`, `authority_scope`, `actor_identity` — no Guard inputs.
- `embedding_ref`, `secret_ref` — no companion-store references.
- `lock` — no distributed locking.
- Everything from `0002_extended_primitives.sql` (16 tables) and `0003_ai_native_and_reliability.sql` (30 tables) — capsules, trust, throttles, circuits, drift, ingress, model routes, prompt versioning, etc.

### Per-operation status

`repo.rs` exposes the canonical write/read surface, all `impl Storage` only:

- `insert_workspace`, `get_workspace` (lines 11-36)
- `insert_actor`, `get_actor` (lines 39-83)
- `insert_session` (lines 86-108)
- `append_event` + `last_event_hash` (lines 111-176) — Chronicle + hash-chain
- `insert_command` (lines 179-203)
- `idempotency_lookup`, `idempotency_record` (lines 206-250)
- `events_in_session` (lines 253-322)

**Same operation on Postgres**: none of these have a `PgStorage` counterpart. The backend.rs documentation (lines 13-18) frames the gap as "dialect translation (`?` → `$N`, `INSERT OR IGNORE` → `ON CONFLICT DO NOTHING`)" — that understates it. The typed API surface for PG is absent, not just placeholder syntax.

### Who actually calls the backend

`StorageBackend::sqlite_pool()` (`backend.rs:49-56`) returns `NotImplemented` when the backend is PG. Call sites that funnel through it: 20 in `crates/actant-command/src/lib.rs` (lines 402, 440, 480, 503, 509, 564, 575, 580, 624, 635, 640, 681, 692, 762, 802, 825, 830, 875, …). Plus `Engine::storage()` (`actant-command/src/lib.rs:88-97`) **panics** if the backend is Postgres.

Every other crate that touches storage takes `&Storage` (SQLite) by type, not `&StorageBackend`:

- `actant-server` (`src/lib.rs:49`, `:68`, `:745`, `:1276`)
- `actant-memory` (`src/lib.rs:15`, `:20`)
- `actant-lock` (`src/lib.rs:22`, `:63`)
- `actant-replay` (`src/lib.rs:59`, `:100`, `:160`, `:218`, `:246`, `:282`, `:304`, `:332`)
- `actant-flow` (`src/lib.rs:392`)
- `actant-ingress` (`src/lib.rs:26`)
- `actant-tenant` (`src/lib.rs:24`, `:29`)
- `actant-audit-export` (all five public fns)
- `actant-effects`, `actant-compensation`, `actant-drift`, `actant-trigger`

So even if `Engine` learned to dispatch through PG, the rest of the substrate could not follow without a sweeping signature change.

### What `PG_NOT_IMPLEMENTED_HINT` actually skips

Defined `backend.rs:27-29`. Surfaced from `Engine::sqlite_storage()` (`actant-command/src/lib.rs:118-125`) on **every command dispatch**. So when an operator points the engine at PG, every command call returns `ActantError::NotImplemented` with the roadmap pointer. The `Engine::postgres()` constructor exists (`actant-command/src/lib.rs:65-67`) but is unusable for any actual work.

### Migrations parity check

Migration tracking exists for both: `Storage::run_migrations` (`lib.rs:122-152`) and `PgStorage::run_migrations` (`postgres.rs:44-81`) both maintain a `schema_migration(name, applied_at)` table, parse the embedded `MIGRATIONS` / `PG_MIGRATIONS` arrays, and idempotently skip applied entries. The runners are structurally identical — strong. **What's missing is a cross-backend parity test.** Nothing fails CI if the PG migration drifts from SQLite. The closest thing is `crates/actant-storage/tests/spec_02_verification.rs`, which checks the SQLite migration against `specs/02-data-model.sql` but does not look at `migrations/pg/`.

**To close**: add `tests/pg_parity.rs` that loads both migration sets, extracts table names + columns, and asserts equality (modulo type translation table).

---

## 3. Object / artifact storage

**There is no object-storage abstraction in the implementation.** The README claims and agent docs reference one; the code stores everything inline.

### How `artifact.uri` actually gets used

The only artifact writer in the substrate is `write_report_event_and_artifact` (`crates/actant-server/src/lib.rs:1275-1321`). It:

1. Appends an `agent_event` with the report content in `payload_inline` (event row holds the bytes).
2. Inserts an `artifact` row with `uri = format!("actantdb://event/{}", event_id)` (line 1311) — a back-reference, not an external URL.
3. `content_hash = sha256_hex(content)` (line 1301) — hashed, not addressed.

The reader (`event_id_from_uri`, lines 1323-1325) strips the `actantdb://event/` prefix and looks the payload up in `agent_event.payload_inline`. **The `artifact` table is effectively a metadata index over event rows; there is no separate blob store.** A `payload_ref / body_ref / input_ref / result_ref / output_ref` column exists on `agent_event`, `message`, `command_record`, `effect`, `effect_result` (`migrations/0001_initial.sql`), but no crate writes a value other than `NULL` or this back-reference string.

### What's claimed vs what's there

- `crates/actant-audit-export/README.md:7` — "A `Destination` trait with implementations: local filesystem, S3-compatible, GCS, Azure Blob." **None exist.** `crates/actant-audit-export/src/lib.rs:177-197` defines `nightly_export(F)` where `F: Fn(&str) -> Result<Box<dyn Write + Send>, _>` — the destination is whatever closure the caller passes. The in-repo test (`lib.rs:284-328`) wires it to an in-memory `HashMap<String, Vec<u8>>`. No S3, GCS, or Azure client is referenced anywhere in the workspace.
- `agents/actant-sync.md:50` — refers to `crates/actant-sync/src/destinations/object_store.rs`. **That file does not exist.** `actant-sync` is one file (`lib.rs`, 58 lines) containing a single function `missing_in` that diffs two `Vec<AgentEvent>` — no engine, no destinations, no trait, no S3.
- `planning/deployment-playbook.md:100` — references "customer-owned S3 buckets" for BYO deploys. No code path lands data there.
- `agents/phase-6-extensions.md:57` — claims `actantdb cluster export --window <ISO> --to <s3://...>`. The CLI (`crates/actant-cli/src/main.rs`) has no `cluster` subcommand; `Backup { to: PathBuf }` only accepts a local path (line 33).

### Search audit

- `aws-sdk` / `aws_sdk_*` — **not present** in `Cargo.toml` of any crate.
- `object_store` crate — **not present**.
- `azure-*`, `gcs::`, `google-cloud-*` — **not present**.
- `MinIO`, `presign`, `bucket` (as code) — **not present**. The only `bucket` hits are in unrelated docs.

**To close**: add `actant-blob` (or a `BlobStore` trait in `actant-storage`) with a `LocalFs` impl now and `S3` / `GCS` / `Azure` impls behind features. Wire `artifact` writers to put bytes there and store the returned URL in `uri`. Update the `*_ref` write paths in `agent_event` / `message` / `effect` / `command_record` to switch on size threshold (inline ≤ N KB, blob ref otherwise). The size threshold is currently absent — every payload is `payload_inline`, which is fine for chat but will balloon the SQLite file on tool-call/result-heavy workloads.

---

## 4. IPFS / content-addressed storage

None. Searches for `ipfs`, `kubo`, `unixfs`, `multihash`, `cid::` return no hits in `crates/` or `packages/` (excluding unrelated three-letter matches like `cid` as a local variable name in `crates/actant-server/tests/memories_endpoint.rs:134`).

The hash chain (`agent_event.event_hash`, `payload_hash`) uses `sha256_hex` (helper in `actant-core`). It's tamper-evident but not retrievable by hash — there is no `get_by_hash` API, and content lives in `payload_inline` keyed by `id`, not by `payload_hash`. Adding content-addressed retrieval would mean (a) building the `BlobStore` from §3, (b) keying it by `content_hash`, and (c) deduping on insert. This is a logical add-on once the blob store lands; it doesn't exist today.

---

## 5. Backup / restore

`actantdb backup` and `actantdb restore` are implemented in `crates/actant-cli/src/main.rs:124-153`.

### `backup`

```
Storage::open → PRAGMA wal_checkpoint(TRUNCATE) → drop(storage) → std::fs::copy(db_path, to)
```

Lines 124-135. A consistent-snapshot file copy. Strong for what it is.

### `restore`

```
warn if exists → mkdir -p parent → std::fs::copy(from, db_path) → Storage::open (sanity check)
```

Lines 136-153. Notes:

- **Does not refuse to overwrite a live DB** — only prints `"warning: overwriting existing database at …"` to stderr (line 141-145). The TODO at line 137-139 acknowledges this: "Refuse to overwrite a live database without explicit force in a future iteration."
- No locking against a concurrent `actant-server` writer; if a server is up, this races.
- Restored file is opened immediately for a sanity check, which will replay migrations against it if the embedded set has advanced beyond the backup — but the runner is idempotent, so a forward-only upgrade just adds the new entries (`lib.rs:122-152`).

### What's not there

- **No PITR.** No WAL shipping, no continuous archival, no `wal_log` / barman / pgBackRest hook.
- **No incremental backup.** Each invocation is a full file copy.
- **No remote target.** `--to` accepts a `PathBuf` only.
- **No Postgres path.** `backup` opens `Storage` (SQLite) unconditionally; pointing `--db` at a `postgres://…` URL would fail at path-parse time.
- **No verification of restore integrity** beyond "Storage::open succeeded." No hash-chain replay; the audit-export crate has the read-side primitives but is not wired in.

`crates/actant-cli/tests/backup_restore.rs:76` notes the byte-for-byte equality the WAL checkpoint enables — the test confirms the copy is exact for an idle DB. There is no test for backup-during-write or restore-over-live.

**To close (priority order)**: (1) hard-refuse `restore` if the target file is opened by another process or has a `-wal` companion; (2) add `--remote s3://…` via the `BlobStore` from §3; (3) add `actantdb verify` that re-walks the hash chain on a restored DB; (4) Postgres path via `pg_dump` shell-out + a documented restore flow.

---

## 6. Concurrent writers + replication

### Writer model

- **SQLite** (`crates/actant-storage/src/lib.rs:88-114`): WAL journal mode (line 98), `Normal` synchronous (line 99), foreign keys on (line 100), pool default `max_connections: 8` (line 74) for file mode and `1` for in-memory (line 65). WAL allows concurrent **readers** while a writer holds the write lock; writes serialize per file. `max_connections: 8` does not change writer concurrency. **The substrate is single-writer per database file.**
- **Postgres** (`postgres.rs:23-36`): pool `max_connections: 8` hardcoded (line 29). Multi-writer in principle, but inactive — see §1: no repo methods, no real call sites.

The single-writer assumption is not stated in code comments, but it's baked in: `actant-lock` (`src/lib.rs:34-60`) acquires by `INSERT` and treats any error as "lock held," which only works because writes serialize. `actant-effects` uses `INSERT OR REPLACE` and `INSERT OR IGNORE` (`src/lib.rs:97, 116`) — SQLite-only syntax that won't translate without rewrite. The `idempotency_record` insert uses `INSERT OR IGNORE` (`repo.rs:234`) for the same reason.

### Replication

- **`actant-sync`** (`crates/actant-sync/src/lib.rs`, 58 lines total): one function `missing_in(a, b) -> Vec<EventId>` that set-diffs two in-memory event lists. **There is no engine, no destination trait, no wire protocol, no `SyncEngine`, no `SyncDestination`.** The README (`crates/actant-sync/README.md:7`) and `agents/actant-sync.md:18-31` describe a `SyncEngine` + `SyncDestination` trait + HTTP/object-store destinations; the code is the one function. The agent doc references `crates/actant-sync/src/destinations/object_store.rs` (line 50) — does not exist.
- **`actant-subscribe`**: live row push to clients (server → client streaming). Not server → server replication. README line 3: "live row replication to clients." This is application-layer pub-sub, not storage-layer replication.
- **No streaming replication path.** No Postgres logical decoding hooks, no read-replica fan-out, no SQLite WAL shipping.
- **No cross-region story.** `planning/performance-budgets.md:73` acknowledges this: "Cross-region replication. Phase 6 adds that."

### Helm claims

`deploy/helm/actantdb/README.md:6` — "`storage.backend=postgres` — multi-replica capable." Combined with the headline finding (server ignores the URL env), this overstates by two layers: the operator sets the flag, the chart provisions Postgres, the pod runs SQLite, and SQLite is single-writer anyway. The `replicaCount: 1` default in `values.yaml:2` would silently be correct; `replicaCount > 1` would produce N pods each writing their own SQLite file, with the Postgres sidecar idle.

---

## 7. What's strong

Naming and moving on:

- Hash-chained `agent_event` with `event_hash` + `payload_hash` (`repo.rs:111-176`). Tamper-evident in both backends' schemas.
- Idempotency tracking with `(workspace_id, idempotency_key)` PK (`migrations/0001_initial.sql:374`, `repo.rs:206-250`). Both backends carry the table.
- Migration ledger via `schema_migration` table, structurally identical in both backends (`lib.rs:122-152`, `postgres.rs:44-81`).
- WAL journal mode, foreign keys on, `Normal` sync — sane SQLite defaults (`lib.rs:95-101`).
- Lease-based locks with TTL sweep on every acquire (`lock/src/lib.rs:34-60`).
- Comment-stripping multi-statement runner (`lib.rs:165-193`, mirrored in `postgres.rs:94-105`).
- `crates/actant-storage/tests/spec_02_verification.rs` enforces in CI that the SQLite migration matches `specs/02-data-model.sql` table-for-table.
- Backup uses `wal_checkpoint(TRUNCATE)` before file copy — byte-identical snapshot for an idle DB.

---

## 8. Highest-impact gaps for production

In rough priority:

1. **Server ignores `ACTANTDB_DATABASE_URL`.** `crates/actant-server/src/lib.rs:2011-2025` always builds SQLite. Fix: read the env var, branch to `PgStorage::open` when set, fail loudly if both `--db` and a URL are present. Today: Postgres deployments are fiction.
2. **`PgStorage` has no repo methods.** Add `impl PgStorage` mirrors for every method in `repo.rs`. Or refactor `repo.rs` around a trait so call sites can take `&dyn Repo`. Then sweep the 12 non-command crates that take `&Storage` to take `&dyn Repo` (or the backend enum) instead.
3. **Postgres schema is 7 of 87 tables.** Port `0001_initial.sql §6-14`, `0002`, and `0003` to `migrations/pg/` with type translations. Add `tests/pg_parity.rs` that fails CI on drift.
4. **No object storage anywhere.** All payloads live in `payload_inline` (TEXT/JSONB) on the event row. There is no size threshold and no `BlobStore` trait. A multi-MB artifact (screenshot, transcript) goes straight into the SQLite file. Land an `actant-blob` crate with `LocalFs` + `S3` + size-threshold routing in artifact / message / effect writers.
5. **Audit-export's destinations are README-only.** Either delete the S3/GCS/Azure claim from `crates/actant-audit-export/README.md:7` or implement a `Destination` trait. Same for `actant-sync` (`README.md:7` + `agents/actant-sync.md:50`).
6. **`restore` does not refuse a live DB.** `crates/actant-cli/src/main.rs:140-145` just warns. Add a flock check or refuse if `<db>-wal` exists.
7. **No backup verification.** A restored DB is sanity-opened but its hash chain is not re-walked. Add `actantdb verify --db <path>` that scans `agent_event` in order and recomputes `event_hash` against `(parent_hash || payload_hash || metadata)`.

Gaps #1, #2, #3 are coupled: #1 makes Postgres reachable, #2 makes it functional for commands, #3 makes it functional for everything else. All three must land together for a Postgres deployment to mean what the docs say it means. #4 is independent and bites any deployment that handles binary artifacts.
