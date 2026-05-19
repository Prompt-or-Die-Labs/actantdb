# Testing — end-to-end coverage matrix

Snapshot of every kind of test ActantDB runs through, as of `0.0.9` (2026-05-18).

## Counts

| Suite | Passing | Method |
| --- | ---: | --- |
| Rust unit + integration | **331** | `cargo test --workspace` |
| TypeScript (vitest) | **25** | `pnpm -r test` |
| Python | **10** | `python3 -m unittest discover -s sdks/python/tests` (+ 1 skipped that needs `ACTANTDB_TEST_URL`) |
| Swift | **62** in 12 suites | `swift test --package-path sdks/swift` |
| Workspace smoke | **1** | `pnpm smoke` (boots Studio + appends + replays end-to-end) |
| **Total** | **429** | — |

CI runs every layer on every push: `.github/workflows/ci.yml` covers Rust on
ubuntu/macOS/windows, TS on the same matrix at Node 24, Python on Linux,
Swift on macOS, plus `helm lint`, the spec-verification gates, and the
agent-package format gate.

## End-to-end scenarios (manual, real-binary, real-package)

Each scenario was run against the actual published npm package + the
release-binaries `actantdb` / `actantdb-server` (not the in-repo `cargo run`
artifact) so it represents what a real user gets.

### Scenario 1 — Approval flow (`@actantdb/mastra` + policy)

Tools have `require_approval: true`. Three sub-runs in one process:

- No approver attached → call is **denied** automatically with reason
  `"no approver attached and autoApprove=false"`.
- `autoApprove: true` → call goes through.
- Custom `resolveApproval` that denies amount > $100 → small refund approved,
  large refund denied.

Captured ledger event kinds: `agent_run_started, user_message_received,
model_call, tool_call_requested, guard_verdict, approval_required,
approval_decision, tool_call_started, tool_call_completed, agent_run_finished`.
Four approval records in the queue, 2 approved + 2 denied — matches the runs.

### Scenario 2 — Replay with overrides (`@actantdb/replay`)

Original run captured. Find the `tool_call_requested` decision point. Three
replays from the same event:

- No overrides → 10/10 entries `identical`.
- Stricter policy via `tighten(policy, { deny: [...] })` → 9/10 identical,
  **1 flipped** (the `guard_verdict` event). Confirms the policy override
  propagates through `runFromEvent` + `diffReplayAgainstOriginal`.
- `alternatePlannerOutput` set → 10 events with the planner-output edge
  rewritten.

### Scenario 3 — 20 concurrent sessions (chain-integrity stress)

20 `wrapped.run(...)` calls in `Promise.all`. Each one writes a
hash-chained run. Captured 160 events total (8 per session × 20 sessions).
Wall-clock: 107 ms total, 5.4 ms per session on average. Chain hashes for
every event are valid 64-char hex.

### Scenario 4 — Server mode (`@actantdb/sdk` against `actantdb-server`)

```bash
actantdb migrate --db /tmp/s4.db
actantdb-server --bind 127.0.0.1:4570 --db /tmp/s4.db
```

`new ActantClient({ baseUrl, workspaceId, actorId })` →
`create_session` → `append_user_message` → `request_tool_call`. All
succeed. Pre-fix, the first call returned `500: storage error: error
returned from database: (code: 787) FOREIGN KEY constraint failed` because
the consumer-chosen actor row didn't exist. Closed in 0.0.8 by adding
actor bootstrap to `CommandEngine::dispatch()`.

### Scenario 5 — Failure injection

Tool throws mid-run. Run 2 fails, but:

- Ledger correctly records `tool_call_completed` with `status=error,
  result.error="upstream 503 timeout"`.
- Run 3 (next call after the failure) runs cleanly — no chain corruption.
- Event-kind distribution across all 3 runs is consistent: 3 agent_runs ×
  8 events each.

### Scenario 6 — Idempotency keys (server mode)

Same `create_session` dispatched twice with the same `idempotencyKey`:

- First call → returns a real `session_id`.
- Second call → returns `{ idempotent_replay: true }`, no new session.

### Scenario 7 — Swift consumer via `ActantDBSupervisor`

`/tmp/actantdb-swift-trial/` — fresh SwiftPM package that downloads the
released binary, uses `ActantDBSupervisor` to spawn it, hits the API
through both the wire `ActantClient` and the high-tier
`Session<ChatMsg>` facade:

- Supervisor finds the binary via explicit `binaryPath:`, polls
  `/v1/healthz/ready`, returns the listening URL.
- `client.healthzReady().isHealthy == true`.
- `createSession` + `appendUserMessage` work end-to-end against the real
  HTTP server.
- `Session<ChatMsg>` round-trips a 3-message transcript through the
  ledger (1 user + 1 assistant + 1 user).

## Benchmarks

Full table in [`BENCHMARKS.md`](./BENCHMARKS.md). One-line summary:

> HTTP single-session **p50 464 µs / p95 1.00 ms / p99 2.12 ms**, 1.8k req/s.
> 10-concurrent aggregate **3.9k req/s**. 200-event replay **3.4 ms**.
> Server RSS only +1.4 MB per 10k events. Disk **1.4 kB/event** steady.

## What is *not* yet tested at this level

Surfaced from the trial runs and filed as GH issues #1–#4:

- Policy DSL numeric comparators (e.g. `amount > 100`). Currently
  string-pattern only; numeric thresholds have to be enforced in the
  agent code instead of the policy.
- Per-framework integration tests (Mastra, LangGraph, OpenAI Agents).
  `@actantdb/mastra` works against any tools-record-shaped agent, but
  framework-specific integration tests don't exist yet.
- Postgres backend. `PgStorage` exists but the command engine still
  hard-codes the SQLite path (per GAPS.md #5). Postgres works for raw
  storage; full-stack server-against-Postgres isn't tested.
- Multi-tenant cross-workspace boundary at scale. The unit-level cross-
  tenant guards have tests; an adversarial-load run hasn't been done.
