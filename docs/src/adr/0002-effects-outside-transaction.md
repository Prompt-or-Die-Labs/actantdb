# ADR-0002: Side effects live outside the database transaction

- **Status:** accepted
- **Date:** 2026-05-17
- **Deciders:** ActantDB Authors

## Context

Agent actions cause real-world side effects: calling models, running shell commands, clicking browsers, sending HTTP, writing files. A naïve approach is to perform the side effect inside the database transaction that schedules it.

That approach has three structural problems:

1. **Roll-forward divergence.** A failed I/O rolls back the database, but the external world already changed.
2. **Roll-back divergence.** A succeeded I/O commits, but a downstream commit failure leaves a half-applied world.
3. **Long-held transactions.** Model calls and browser sessions can take seconds or minutes. Holding a database transaction open for that long blocks other writers.

`/specs/05-security-model.md` §2 invariant 4 states: **No side effect inside a database transaction.**

## Decision

ActantDB separates **scheduling** from **execution**:

- Commands run inside a transaction and insert an `effect` row that records the intent to perform a side effect.
- **Workers** (registered actors with `worker_capability`) claim effect rows after the transaction commits, perform the work outside the database, and call `complete_effect` to write the result back.

The full lifecycle is in `/specs/04-effect-protocol.md`.

This decomposition forces idempotency (workers may be retried; effects carry `idempotency_key`), but in exchange we get:

- The world and the database stay in lockstep through `effect_result` rows.
- Approval gates (Guard) can pause execution between schedule and run without holding a transaction open.
- Long-running effects (browser sessions, streaming model calls) are first-class.
- Workers can run anywhere — local sandbox, remote node, attested enclave — without altering the database protocol.

## Consequences

### Positive

- Database transactions are short and predictable.
- Replay can reuse `effect_result` rows without re-executing.
- Approval can interrupt execution.
- Workers can scale independently.

### Negative

- Idempotency becomes mandatory at the worker layer. `/specs/04-effect-protocol.md` §3 specifies the rules.
- "Did this happen?" is answered by checking `effect.status` plus `effect_result`, not by a single row.
- An effect that the worker performs but fails to acknowledge can theoretically be re-executed by a successor worker. Mitigated by external-API idempotency keys and per-worker de-dupe ledgers.

### Neutral / open

- The set of effect types grows; new types need worker capabilities and policy bindings. Catalog lives in `/specs/04-effect-protocol.md` §7.

## Alternatives considered

- **Two-phase commit with external systems.** Rejected — most external systems do not participate in XA; treating them as if they did is wishful thinking.
- **Outbox pattern with a single bus.** Effect rows ARE an outbox; the worker protocol is the consumer. This decision is consistent with that pattern, with worker leases and heartbeats added for safety.
- **Saga orchestration as the primary abstraction.** Rejected as the primary abstraction; sagas are emergent from workflows (`actant-flow`) on top of the effect queue. Workflows are users of the queue, not its replacement.

## References

- `/specs/04-effect-protocol.md`
- `/specs/05-security-model.md` §2 invariant 4
- `/specs/01-architecture.md` §"Effect Engine"
