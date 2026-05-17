# ADR-0016: Reliability primitives are built into ActantDB

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Autonomous agents run for minutes to months. They retry, queue work, hit provider rate limits, exhaust budgets, encounter circuit-breaking failures, lock shared resources, and ingest external events. Without a backend that owns these primitives, every team rebuilds:

- ad-hoc retry loops with subtle bugs,
- in-process rate limiters that don't fan out,
- silent dead-letter handling,
- duplicate side effects from missing idempotency,
- broken workers that hold leases forever,
- locks implemented in Redis-by-convention,
- inconsistent webhook authentication.

A backend that calls itself an "autonomous-action substrate" cannot externalize these. They are the substrate.

## Decision

ActantDB ships **all** of the following as first-class subsystems with their own crates, tables, CLI commands, and tests:

- **Throttle** (`actant-throttle`) — multi-axis rate limits.
- **Queue** (`effect_queue_entry` in `actant-effects`) — priority + fairness + backpressure.
- **Retry** (`retry_policy` in `actant-effects`) — declarative retry with backoff + jitter + retry-on lists.
- **Lease** (`effect_claim` extended in `actant-effects`) — input-hash-bound worker mandates.
- **Schedule** (`actant-trigger`) — cron, interval, delay, event, webhook, manual.
- **Budget** (`actant-policy` + `budget` table) — tokens, cost, time, calls, approvals, file writes.
- **Circuit** (`actant-circuit`) — closed/open/half_open/degraded per dependency.
- **Cache** (`actant-cache`) — semantic + model + retrieval + tool-result + artifact-summary.
- **DLQ** (`dead_letter_item` in `actant-effects`) — preserved failed work convertible to evals.
- **Lock** (`actant-lock`) — lease-bounded resource locks.
- **Ingress** (`actant-ingress`) — HMAC-verified webhooks + email + calendar + fs + MCP + A2A.
- **Idempotency** (`idempotency_record`, universal) — every command + effect.

Defaults catalog (`/specs/18-reliability-primitives.md` §9) ships in every scaffolded project's `actant.yaml`.

The integration order at runtime (`/specs/18-reliability-primitives.md` §10):

```
Command → Guard → Throttle → Budget → Circuit → Queue → Lease → Worker → Retry → Observation → DLQ → Subscribers → Trace
```

Every step is replayable.

## Consequences

### Positive

- Developers get reliability primitives without rebuilding them.
- Failures become structured rather than silent — every dead letter, every circuit-open, every rate-limited request is a row.
- The CLI exposes these as ordinary surfaces (`actant throttle`, `actant circuit`, `actant dlq`, `actant queue`) so operating an agent fleet doesn't require a different toolset from running it locally.
- The defaults catalog gives sane production behavior on day one.

### Negative

- Schema growth: a dozen new tables in migration 0003. Mitigated — all additive, all scoped under existing concepts.
- Performance: every enqueue runs Throttle + Budget + Circuit. Mitigated — these checks are in-memory after the first lookup; under 1ms at p99.
- Surface area: many CLI commands. Mitigated — `actant doctor` surfaces the relevant subset for a project's actual configuration.

### Neutral / open

- Whether to add a "rate-limit-as-a-service" external mode (federate rate-limit state across cluster nodes) is a Phase 6 decision per `/planning/phase-6-plan.md`.
- Cache hit-rate metric defaults are TBD; will be tuned in Phase 3 once retrieval evals exist.

## Alternatives considered

- **Compose with Temporal / etc.** Rejected — would force ActantDB to be one subsystem among many, breaking the unified causal graph and replay story.
- **Ship only retry + queue; outsource the rest.** Rejected — partial coverage forces developers to hand-build the missing pieces, exactly the failure mode this ADR addresses.
- **Make every primitive optional.** Rejected — the defaults catalog is the developer contract.

## References

- `/specs/18-reliability-primitives.md` — full design.
- `/agents/actant-throttle.md`, `/agents/actant-circuit.md`, `/agents/actant-lock.md`, `/agents/actant-ingress.md`.
- Prior art: Temporal workflows + activities, AWS SQS DLQ, Cloudflare Durable Objects.
