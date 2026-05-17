# Worker fleet — cross-phase architecture

A single design doc for the worker fleet across Phase 2 → Phase 6. Per-worker specifics live in `agents/actant-worker-*.md`.

## Roles

| Worker                    | Phase | Lease types                                                |
| ------------------------- | ----- | ---------------------------------------------------------- |
| `actant-worker-shell`     | 2     | `shell.run` (and `tool.call` of kind=shell)                |
| `actant-worker-file`      | 2     | `file.read`, `file.write`                                   |
| `actant-worker-model`     | 2     | `model.call`                                                |
| `actant-worker-mcp`       | 2     | `tool.call` of kind=mcp                                     |
| `actant-worker-browser`   | 6     | `browser.act` (deferred from Phase 4 to keep Phase 2 lean)  |
| `actant-worker-http`      | 6     | `http.request`                                              |
| `actant-worker-embed`     | 3     | `memory.embed` (runs locally via `actant-embed`)            |
| `actant-worker-replay-*`  | 5     | Replay-scoped variants of each above for `mode=experimental`|

## Identity and auth

- Every worker is an `actor` row with `kind='worker'`. The server mints a long-lived bearer token (or mTLS cert) for each worker on registration.
- A worker token is *not* a user token. It can only call the worker API endpoints + `register_worker` / `worker_heartbeat`.
- The token's scope is bound by the worker's declared capabilities. A shell worker cannot claim a `model.call` lease even if it tries.

## Sandboxing rules

| Kind     | Default sandbox             | Network    | FS                           |
| -------- | --------------------------- | ---------- | ---------------------------- |
| shell    | OS-level (sandbox-exec / bwrap / JobObject) project-rooted | denied | project-rw or project-ro |
| file     | path-pattern enforced       | denied     | resource_pattern only        |
| model    | none (HTTP only)            | allowlist  | no FS                        |
| mcp      | per-transport               | allowlist  | per-transport                |
| browser  | dedicated browser profile   | allowlist  | no host-FS access            |
| http     | none                        | allowlist  | no FS                        |
| embed    | none                        | local-only | embedding store path         |
| replay-* | inherit base + replay-scope | overridden | replay-scope                 |

The sandbox profile per lease is named in `effect_claim.sandbox_policy_ref`. Workers refuse leases whose sandbox profile they cannot enforce.

## Lifecycle

```
worker boot
  ├─ load config (server URL, token, name, capabilities)
  ├─ register_worker (capabilities, host, version)
  └─ start heartbeat loop
           │
           ▼
worker main loop
  ├─ claim_effect (up to N parallel)
  ├─ for each lease:
  │    ├─ verify input_hash
  │    ├─ start_effect
  │    ├─ execute under sandbox
  │    ├─ stream observations
  │    └─ complete_effect (with final_input_hash)
  └─ on shutdown: drain leases, mark draining, then offline
```

## Conformance suite

A test harness in `crates/actant-effects/tests/worker_conformance.rs` (added Phase 2) drives a mock server and asserts every reference worker satisfies the protocol. Third-party workers can run the same harness to self-certify.

Conformance check list:

- claim returns valid `Lease` rows.
- heartbeat extends the lease.
- lease loss after missed heartbeats produces clean re-claim with `attempt_count += 1`.
- `complete_effect` is idempotent on `effect_id`.
- `final_input_hash` mismatch rejected.
- observations stream during long-running effects.
- on registered capability mismatch, the lease is rejected without state mutation.

## Worker deployment

| Mode                | Where                                          |
| ------------------- | ---------------------------------------------- |
| Embedded            | Inside `actantdb-server` (Phase 1 stub)        |
| Sidecar             | Same host as server (Phase 2 default)          |
| Remote local        | On the user's laptop, reaching cloud server    |
| Sandboxed cluster   | Kubernetes pods with strict NetworkPolicies    |
| Attested enclave    | Phase 6+ for high-sensitivity workloads        |

## Worker scaling and routing

- Per-effect-type concurrency limits server-side (`worker_capability`) and worker-side (env config).
- The server's claim selection is FIFO by `effect.created_at` within type, filtered by `next_attempt_at`.
- Workers preferring certain effects (e.g. local model worker for `local_only` leases) declare it as metadata; the scheduler can prefer matching workers but cannot guarantee — leases never wait for a specific worker.

## Worker observability

Every worker emits the following metrics (Phase 6 scraped by Prometheus / OpenTelemetry):

- `actant_worker_leases_claimed_total{kind}`
- `actant_worker_leases_completed_total{kind,outcome}`
- `actant_worker_lease_duration_seconds{kind}`
- `actant_worker_heartbeats_total{kind,outcome}`
- `actant_worker_lease_losses_total{kind}`
- `actant_worker_dedupe_hits_total{kind}`

Studio's Workers panel renders these.

## Worker security incidents

| Incident                                                  | Detection                          | Response                              |
| --------------------------------------------------------- | ---------------------------------- | ------------------------------------- |
| Worker tries to claim outside its capabilities            | Server-side check                  | 403, increment a security counter      |
| Worker reports `final_input_hash` ≠ lease.input_hash      | Server rejects `complete_effect`   | Effect goes to `failed`, regret filed |
| Worker missed N consecutive heartbeats                    | Reaper                             | Re-claim by another worker            |
| Worker leaks a secret in an observation                    | Sensitivity classifier             | Quarantine artifact, regret + alert   |
| Worker version drift from server                          | `worker.version` mismatch          | Worker refuses leases until upgrade   |
