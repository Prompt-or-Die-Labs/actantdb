# Work package: `actant-audit-export`

## Context

Nightly export of Chronicle slices for compliance and external audit. Phase 6.

## Specs to read first

- `/specs/05-security-model.md` §8 (privacy and deletion semantics).
- `/specs/11-roadmap.md` Phase 6 decision gate.
- `/specs/13-actant-contract.md` §22 (audit obligation).

## Scope

```rust
pub struct ExportPlan {
    pub workspace_id: WorkspaceId,
    pub window_start: OffsetDateTime,
    pub window_end:   OffsetDateTime,
    pub sensitivity_ceiling: Sensitivity,
    pub retention_policy: RetentionPolicy,
}

#[async_trait]
pub trait Destination: Send + Sync {
    async fn put(&self, path: &str, bytes: Vec<u8>) -> Result<(), ExportError>;
    async fn verify(&self, path: &str, hash: &str) -> Result<bool, ExportError>;
}

pub struct Exporter { storage: Arc<actant_storage::Storage>, dest: Box<dyn Destination> }

impl Exporter {
    pub async fn run(&self, plan: ExportPlan) -> Result<ExportManifest, ExportError>;
}
```

### Output format

Per-day directory:

```
ws_<workspace>/<YYYY-MM-DD>/
  events.jsonl          (agent_event rows, one per line, payload tombstoned if past retention)
  commands.jsonl        (command_record rows)
  effects.jsonl
  approvals.jsonl
  manifest.json         (sha-256 of every file, total row counts, plan parameters)
```

### Internal modules

```
crates/actant-audit-export/src/
├── lib.rs
├── plan.rs
├── streams/
│   ├── mod.rs
│   ├── events.rs
│   ├── commands.rs
│   ├── effects.rs
│   └── approvals.rs
├── destinations/
│   ├── mod.rs
│   ├── local.rs
│   └── s3.rs
├── manifest.rs
└── error.rs
```

### Tests

- A re-run against the same `(workspace, window, policy)` produces byte-identical files.
- Tombstoned payloads (past retention) appear with `payload_ref=null`, `payload_hash` intact.
- A row whose sensitivity exceeds the plan's ceiling is excluded entirely (its `id` does not leak).
- Manifest re-hashes match per-file hashes on re-verify.

## Acceptance criteria

- [ ] Build / test / clippy green.
- [ ] Identical bytes on re-run property test (1k rows, 10 reruns).
- [ ] Sensitivity exclusion fuzz tests pass.

## Do NOT

- Do NOT include raw secret material in exports. Secret references appear as `secret_ref.handle` only.
- Do NOT leak counts of rows excluded by sensitivity.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
