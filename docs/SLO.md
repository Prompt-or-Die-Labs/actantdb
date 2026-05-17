# Service Level Objectives — ActantDB v1

Production targets the v1 substrate must meet to be considered ready for
team-grade workloads. Targets are derived from the criterion benchmarks in
[`bench/`](../bench/) and verified in CI on every PR.

## Latency

| Surface | p50 target | p99 target | Bench |
| --- | --- | --- | --- |
| `storage::append_event` (SQLite in-memory) | < 100 µs | < 250 µs | `bench/benches/storage_append.rs` |
| `command::dispatch(append_user_message)` | < 200 µs | < 500 µs | `bench/benches/command_dispatch.rs` |
| `POST /v1/command` end-to-end (loopback) | < 5 ms | < 25 ms | `bench/benches/http_command.rs` |
| `GET /v1/healthz/ready` (SELECT 1 + JSON) | < 1 ms | < 5 ms | covered by `health_probes` test |
| `WS /v1/ws` broadcast fan-out (1 subscriber) | < 10 ms | < 50 ms | covered by `ws_subscription` test |

## Throughput

| Surface | Floor | Bench |
| --- | --- | --- |
| `POST /v1/command` sustained | 1 000 RPS | `bench/benches/http_command.rs` |
| `storage::append_event` sustained | 10 000 EPS | `bench/benches/storage_append.rs` |

## Availability

| Component | Target | How |
| --- | --- | --- |
| `/v1/healthz/live` | 99.95 % | Liveness probe; pod-restart on failure |
| `/v1/healthz/ready` | 99.9 % | Readiness probe gates traffic |
| `/v1/command` 5xx rate | < 0.1 % | Structured error responses; rate limit returns 429, not 5xx |
| `actantdb-server` startup | < 30 s cold | Helm startup probe `failureThreshold: 30, periodSeconds: 2` |

## Durability

| Property | Guarantee |
| --- | --- |
| Chronicle event durability | Once `POST /v1/command` returns 2xx, the agent_event row is in WAL. |
| Idempotency window | 30 days (default retention; configurable via RetentionPolicy). |
| Backup recovery point | Manual: `actantdb backup --to <path>` after WAL checkpoint. |

## Safety

| Property | Mechanism | Verified by |
| --- | --- | --- |
| Tenant isolation | `assert_event_in_tenant` + every query scoped on `workspace_id` | `actant-tenant` tests |
| No raw secrets in events | secret_ref-only schema | `spec_02_verification::no_table_stores_raw_secret_material` |
| Guard cannot be bypassed | Every tool call goes through `evaluate()` | `spec_03_verification::commands_do_not_perform_io_directly` |
| One worker per effect | UPDATE WHERE status='pending' + rows_affected check | `actant-effects/tests/concurrency.rs` |
| Tampered JWTs rejected | RS256 verify against JWKS | `actant-auth/tests/rs256_round_trip.rs::rs256_rejects_tampered_token` |

## Observability

| Signal | Where |
| --- | --- |
| Per-request `x-request-id` | every HTTP response; verified by `health_probes::request_id_*` |
| Prometheus metrics | `/v1/metrics`: `actantdb_events_total{event_type}`, `actantdb_effects_total{status}`, `actantdb_approvals_pending`, `actantdb_workspaces_total` |
| Structured logs | `tracing::info!(request_id=...)` on every request |
| Chronicle | `agent_event` hash-chained per session; tamper-evident |

## Out-of-scope for v1

These belong to v1.5+ (post-design-partner):

- Multi-region replication
- Cross-tenant audit aggregation
- Real-time anomaly detection
- Cost-attribution rollups across workspaces
