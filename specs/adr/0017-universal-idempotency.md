# ADR-0017: Idempotency is required for every command and effect

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Agents retry. Workers crash mid-effect. Networks drop responses. Humans double-approve. Providers return partial successes that look like failures.

Without idempotency, every retry risks duplicate side effects: duplicate emails, duplicate GitHub issues, duplicate file patches, duplicate payments, duplicate memory candidates, duplicate workflow runs. These are exactly the failures users will not forgive in an "autonomous-action substrate."

The Phase 1 effect protocol already requires `effect.idempotency_key`. This ADR generalizes the rule:

> Every command and every effect carries an idempotency key. Repeated submissions return the original result.

## Decision

ActantDB enforces idempotency universally:

1. **Every command** accepts an `Idempotency-Key` header (HTTP) or `idempotency_key` field (SDK). Server inserts an `idempotency_record` keyed by `(workspace_id, idempotency_key)`. Duplicate submissions return the recorded result.
2. **Every effect** carries an `idempotency_key`. `UNIQUE (workspace_id, idempotency_key)` is already enforced in the `effect` table (Phase 1).
3. **Tools declare `idempotency_required`.** Tools with `idempotency_required=false` and `default_risk_level >= high` cannot be auto-retried — failures escalate to `awaiting_approval`.
4. **Workers use external idempotency keys** when external APIs support them (Stripe, GitHub `X-Idempotency-Key`, etc.). Where the API does not, workers persist a local de-dupe ledger keyed by `effect_id`.
5. **Idempotency is per-actor scoped.** Lookups filter by `actor_id` so one actor's key cannot collide with another's.
6. **Idempotency records expire.** Default 24h window per workspace policy; long-running workflows can extend per workspace.

## Consequences

### Positive

- Duplicate side effects are structurally prevented across all retry paths (worker lease loss, network drop, double-click).
- Replay is sharper: a duplicate command request during replay returns the original result rather than producing divergent state.
- Tools that declare their idempotency posture honestly become trusted; tools that don't are visibly gated behind approval on retry.

### Negative

- Every command request requires a key. SDKs auto-generate when omitted; CLI sets one per command invocation. Documented.
- Storage growth: `idempotency_record` row per command. Mitigated by the expiry policy and a periodic compaction job.

### Neutral / open

- Whether idempotency keys should be globally unique vs per-workspace is settled: **per-workspace**, with the `actor_id` filter for the secondary check. Cross-workspace collisions are impossible by the unique constraint.

## Alternatives considered

- **Idempotency only at the effect layer.** Rejected — commands can also produce duplicate side effects (a duplicate `approve_tool_call` would mint two approvals).
- **Best-effort idempotency.** Rejected — agents and operators must be able to retry without thinking; partial guarantees are worse than no guarantees.
- **Idempotency keys auto-generated and hidden from the developer.** Rejected for CI — explicit keys allow CI scripts to retry safely. SDKs generate them transparently for ordinary calls; explicit keys are an opt-in for scripting.

## References

- `/specs/18-reliability-primitives.md` §8 (Universal idempotency).
- `/specs/04-effect-protocol.md` §3 (Idempotency).
- `/specs/03-command-spec.md` § "Idempotency".
