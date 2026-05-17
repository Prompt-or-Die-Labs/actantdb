# actant-effects

The Effect Engine — queue, claim protocol, retries, dead-lettering.

Owns:

- `EffectQueue::enqueue(effect)` — called from within a command transaction.
- `EffectQueue::claim(worker_id, effect_types, lease_seconds, max_count)` — atomic transition of `pending → claimed` rows.
- Heartbeat + lease-loss reaper.
- Retry/backoff scheduling per `specs/04-effect-protocol.md` §4.
- Dead-letter handling and `human.notify` fan-out hook.
- Idempotency-key enforcement (`UNIQUE(workspace_id, idempotency_key)`).
- Worker registration helpers (`register_worker`, `worker_capability`, `worker_heartbeat`).

Phase 1 scope: queue + claim + heartbeat. Reference workers (model, shell, file) ship in `actant-worker-*` binaries authored from Phase 2 work packages.

See `agents/actant-effects.md` for the work package.
