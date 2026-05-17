# Work package: `actant-circuit`

## Context

Circuit breakers per `(workspace, dependency_key)`. States: closed | open | half_open | degraded. Drives `actant-models` routing fallback and `actant-effects` claim blocking. Emits `circuit_state_changed` events into the chronicle.

## Specs to read first

- `/specs/18-reliability-primitives.md` §4.
- `/specs/adr/0016-reliability-primitives.md`.

## Scope

```rust
pub struct CircuitService { storage: Arc<actant_storage::Storage> }

pub enum State { Closed, Open, HalfOpen, Degraded }

impl CircuitService {
    pub async fn record_outcome(&self, tx: &mut Transaction<'_>, dependency_key: &str, ok: bool) -> Result<State, CircuitError>;
    pub async fn permit(&self, dependency_key: &str) -> Result<bool, CircuitError>;
    pub async fn force_open(&self, tx: &mut Transaction<'_>, dependency_key: &str, reason: &str) -> Result<(), CircuitError>;
    pub async fn reset(&self, tx: &mut Transaction<'_>, dependency_key: &str) -> Result<(), CircuitError>;
}
```

### Default thresholds

```
provider: 5 failures in 60s → open 30s → half_open 10% traffic
mcp:      3 failures in 60s → open 60s
tool.shell: 10 failures in 5m → degraded (high-priority only)
```

### Internal modules

```
crates/actant-circuit/src/
├── lib.rs
├── service.rs
├── thresholds.rs
├── state_machine.rs
└── error.rs
```

### Tests

- Threshold crossing trips the circuit; half_open admits the configured share.
- A closed circuit returning errors increments failure_count atomically under concurrent callers.
- `force_open` is audited.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] Concurrent record_outcome property test (200 callers) maintains correct counts.

## Do NOT

- Do NOT bypass the circuit on retry. If open, the retry is delayed/rejected.
- Do NOT couple to provider names; `dependency_key` is opaque.

## Hand-off

`just ci`.
