# actant-worker-protocol

Shared library every ActantDB worker depends on. Phase 2.

Owns:

- HTTP client for the worker API endpoints (`claim`, `heartbeat`, `start`, `observe`, `complete`).
- `Lease` type carrying `effect_id`, `effect_type`, `input_ref`, `input_hash`, `expires_at`, `permission_scope_ref`, `sandbox_policy_ref`, `max_attempts`.
- Idempotency helpers: per-worker local de-dupe ledger and external-API key plumbing.
- Structured-observation builders (`evidence_type`, `confidence`, `verification_status`).
- Compensation-plan helpers: `pre_state_artifact_ref` capture.
- Worker-side intent / drift hooks: refuse to execute if `input_hash` does not match the lease.

Does **not** own: any specific tool kind. Each worker binary (shell, file, model, mcp) depends on this and supplies its own execution logic.

See `agents/actant-worker-protocol.md` and `planning/worker-fleet.md`.
