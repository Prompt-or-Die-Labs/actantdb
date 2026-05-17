# Work package: `actant-throttle`

## Context

Multi-axis rate limiting. Every command + effect runs through Throttle after Guard and before Queue. Algorithms: token bucket, leaky bucket, fixed/sliding window, concurrency semaphore, weighted fair queue, priority/deadline queues, adaptive provider-rate tracking that ingests `RateLimit-*` HTTP headers.

## Specs to read first

- `/specs/18-reliability-primitives.md` §1 (Throttle), §10 (integration).
- `/specs/adr/0016-reliability-primitives.md`.

## Scope

```rust
pub struct ThrottleService { storage: Arc<actant_storage::Storage> }

pub struct ThrottleRequest<'a> {
    pub workspace_id: &'a WorkspaceId,
    pub scope: ThrottleScope,        // Actor | Agent | Tool | Provider | Workflow | Tenant | ...
    pub key: &'a str,
    pub cost: f32,                   // tokens consumed by this request
}

pub enum Decision { Allow, Delayed { retry_after_ms: u32, fairness_key: Option<String> }, Denied { reason: String } }

impl ThrottleService {
    pub async fn check(&self, req: ThrottleRequest<'_>) -> Result<Decision, ThrottleError>;
    pub async fn record_provider_headers(&self, provider: &str, headers: &ProviderRateLimitHeaders) -> Result<(), ThrottleError>;
    pub async fn set_policy(&self, tx: &mut Transaction<'_>, policy: NewRateLimitPolicy) -> Result<RateLimitPolicyId, ThrottleError>;
}
```

### Internal modules

```
crates/actant-throttle/src/
├── lib.rs
├── service.rs
├── algorithms/                  (token_bucket, leaky_bucket, fixed_window, sliding_window, concurrency, wfq, priority, deadline, adaptive)
├── adaptive.rs                  (provider header parser; RateLimit-Limit / Reset / Remaining etc.)
├── decision.rs
└── error.rs
```

### Tests

- Token bucket: 60/min limit + burst 10 → exactly 10 immediate Allow, then ~1 per second.
- Adaptive: an OpenAI 429 with `Retry-After: 5` produces a `Delayed { retry_after_ms ≈ 5000 }` for the next call.
- Fairness: weighted fair queue across two workspaces with 2:1 weights.
- Property: 10k random requests against a mixed policy set produce no over-limit allow.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] p99 `check()` latency ≤ 1ms in the bench harness.
- [ ] Every algorithm in §1 of the spec has a positive + negative test.

## Do NOT

- Do NOT couple to a specific provider's API; the header parser is generic.
- Do NOT make `check()` mutate without a transaction; persistence is explicit.

## Hand-off

`just ci`.
