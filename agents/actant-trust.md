# Work package: `actant-trust`

## Context

Behavioral trust profiles. Aggregates operational signals into per-(actor, capability) scores. Advisory only — Guard reads it to escalate risk.

## Specs to read first

- `/specs/13-actant-contract.md` §19 (behavioral trust).
- `/specs/14-extended-primitives.md` §10 (trust_profile).
- `/specs/adr/0007-behavioral-trust.md`.

## Scope

```rust
pub struct TrustService { storage: Arc<actant_storage::Storage> }

pub struct Signals {
    pub tool_success_rate: f32,        // 0..1
    pub policy_violation_rate: f32,
    pub approval_denial_rate: f32,
    pub memory_correction_rate: f32,
    pub workflow_completion_rate: f32,
    pub user_feedback_score: f32,
    pub eval_pass_rate: f32,
    pub replay_divergence: f32,
    pub sample_size: u32,
}

impl TrustService {
    pub async fn recalculate(&self, tx: &mut Transaction<'_>, actor: &ActorId, area: &str) -> Result<TrustProfileRow, TrustError>;
    pub async fn pin(&self, tx: &mut Transaction<'_>, actor: &ActorId, area: &str, score: f32, reason: &str) -> Result<(), TrustError>;
    pub fn compute(signals: &Signals) -> (f32, f32);   // (score, confidence)
}
```

The `compute` function is pure: takes signals, returns score + confidence. Weights are configurable per workspace via a policy artifact (Phase 3 ships sensible defaults).

### Internal modules

```
crates/actant-trust/src/
├── lib.rs
├── service.rs
├── compute.rs               // pure scoring
├── signals.rs               // signal extractors that hit storage
├── thresholds.rs            // upgrade/downgrade detection
└── error.rs
```

### Tests

- Pure-function tests on `compute`: known signal combinations produce expected scores.
- Threshold-crossing: signal sequence that crosses `0.4` downward emits `trust_downgrade` exactly once.
- Pin overrides recompute; the `pin_trust` audit event records the reason.

## Acceptance criteria

- [ ] Build / test / clippy green.
- [ ] No public function panics on bad signals (e.g. NaN, sample_size=0); returns `(0.0, 0.0)` with low confidence.
- [ ] Recalibration on 10k synthetic actors completes in < 5s on a laptop.

## Do NOT

- Do NOT have trust grant authority. Advisory only.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
