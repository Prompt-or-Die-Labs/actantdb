# Work package: `actant-worker-protocol`

## Context

Shared library for every Phase 2 worker binary (shell, file, model, mcp). Standardizes the wire protocol with the server, idempotency helpers, observation builders, and compensation-plan capture so each worker only writes its own execution logic. Without this crate, four workers would each reinvent claim/heartbeat/observe — and drift.

## Specs to read first

- `/specs/04-effect-protocol.md` — full file (the wire contract).
- `/specs/14-extended-primitives.md` §2 (Observation), §11 (Compensation plan), §15 (Effect lease rich form).
- `/specs/08-api-spec.md` §6 (Worker API HTTP endpoints).
- `/specs/05-security-model.md` §2 invariants 4, 11.

## Scope

### Public API surface

```rust
pub struct ProtocolClient {
    base_url: String,
    auth_token: String,
    worker_id: WorkerId,
    http: reqwest::Client,
}

pub struct Lease {
    pub effect_id: EffectId,
    pub effect_type: EffectType,
    pub workspace_id: WorkspaceId,
    pub input_ref: Option<String>,
    pub input_hash: String,
    pub idempotency_key: Option<String>,
    pub expires_at: OffsetDateTime,
    pub attempt_number: u32,
    pub permission_scope_ref: Option<String>,
    pub sandbox_policy_ref: Option<String>,
    pub max_attempts: u32,
}

impl ProtocolClient {
    pub async fn claim(&self, effect_types: &[EffectType], lease_seconds: u32, max_count: u32) -> Result<Vec<Lease>, ProtocolError>;
    pub async fn heartbeat(&self, lease: &Lease, extend_seconds: u32) -> Result<(), ProtocolError>;
    pub async fn start(&self, lease: &Lease) -> Result<(), ProtocolError>;
    pub async fn observe(&self, lease: &Lease, obs: NewObservation) -> Result<ObservationId, ProtocolError>;
    pub async fn complete(&self, lease: &Lease, result: EffectCompletion) -> Result<(), ProtocolError>;
}

pub struct NewObservation {
    pub evidence_type: String,        // 'shell_result' | 'file_content' | ...
    pub summary: String,
    pub raw_artifact_ref: Option<String>,
    pub confidence: f32,
    pub sensitivity: Sensitivity,
    pub verification_status: VerificationStatus,
}

pub struct EffectCompletion {
    pub succeeded: bool,
    pub result_ref: Option<String>,
    pub error: Option<EffectError>,
    pub retriable: bool,
    pub final_input_hash: String,    // worker recomputes; protocol asserts equal to lease.input_hash
}

// Per-worker local de-dupe ledger.
pub struct LocalLedger { /* on-disk K-V keyed by effect_id */ }
impl LocalLedger {
    pub fn already_completed(&self, effect_id: &EffectId) -> bool;
    pub fn mark_completed(&self, effect_id: &EffectId, result_hash: &str);
}
```

### Internal modules

```
crates/actant-worker-protocol/src/
├── lib.rs
├── client.rs              // ProtocolClient
├── lease.rs               // Lease + parsing
├── observation.rs         // NewObservation builders
├── completion.rs          // EffectCompletion
├── ledger.rs              // LocalLedger
├── compensation.rs        // pre_state_artifact_ref capture helpers
└── error.rs
```

### Tests

- Round-trip: a claim's `Lease` JSON parses; a `complete` round-trip reaches the test server.
- Input-hash guard: if a worker tries to `complete` with `final_input_hash != lease.input_hash`, the call errors before contacting the server.
- LocalLedger correctness: a re-claim of an already-completed effect short-circuits with the stored result hash.
- Network resilience: heartbeat retries on 5xx with bounded backoff.

## Acceptance criteria

- [ ] `cargo build -p actant-worker-protocol` zero warnings.
- [ ] `cargo test -p actant-worker-protocol` passes.
- [ ] `cargo clippy -p actant-worker-protocol -- -D warnings` passes.
- [ ] Worker conformance harness (separate; lives in tests) accepts at least one reference worker that does nothing but use this library.

## Do NOT

- Do NOT add tool-specific logic. This is the shared library.
- Do NOT implement the four workers here. Separate work packages.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
