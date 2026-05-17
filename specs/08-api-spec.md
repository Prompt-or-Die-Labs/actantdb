# 08 — API Spec

ActantDB exposes a single server with three surfaces:

1. **Command API** — HTTP POST per command type.
2. **Subscription API** — WebSocket for live row updates.
3. **Replay API** — HTTP for replay control (a thin wrapper over commands).

There is no general query API in Phase 1. Reads come through subscriptions (which include a snapshot) and through a small set of read endpoints (`/v1/effects/claim`, `/v1/checkpoints/{id}`). This keeps the surface narrow and forces auditability through the command path.

Sections:

1. Versioning and base URL
2. Authentication
3. Common conventions
4. Command API
5. Subscription API
6. Worker API
7. Replay API
8. Health, version, metadata
9. Errors
10. Rate limiting

---

## 1. Versioning and base URL

```
https://<host>:<port>/v1
```

Versioning is path-prefixed. Breaking changes increment the prefix (`/v2`); additive changes do not.

---

## 2. Authentication

Two equally valid mechanisms:

- **Bearer token** — `Authorization: Bearer <token>`. Tokens are minted by the server's identity flow (Phase 1 ships a static token; Phase 2 adds OAuth/OIDC).
- **mTLS** — for worker fleets and team deployments. The client cert's CN maps to an `actor_identity` row.

The authenticated principal must map to exactly one `actor` row; commands that target a different workspace are subject to cross-workspace authority checks (`05-security-model.md` §10).

---

## 3. Common conventions

### Identifiers

All IDs are opaque strings (ULID/UUIDv7 textual form). Servers and clients never compute on IDs beyond equality.

### Timestamps

RFC3339 UTC with second or millisecond precision (e.g. `2026-05-17T14:30:00Z`).

### Hashing

`SHA-256` hex (lowercase) unless otherwise stated.

### Content negotiation

- Requests: `Content-Type: application/json`.
- Responses: `application/json`.
- For artifact uploads, see §6.

### Idempotency

Commands accept an `Idempotency-Key` header. The server records the key with `command_record` and returns the original response if a duplicate arrives within the workspace's idempotency window (default 24h).

### Compression

`Accept-Encoding: gzip, br` honored.

---

## 4. Command API

### `POST /v1/command`

Single command per request.

```http
POST /v1/command HTTP/1.1
Authorization: Bearer ...
Idempotency-Key: 0a2b...
Content-Type: application/json

{
  "command": "request_tool_call",
  "actor_id": "agent_123",
  "workspace_id": "ws_123",
  "input": {
    "session_id": "sess_123",
    "tool_name": "shell.run",
    "arguments": { "command": "pytest" }
  }
}
```

Successful response:

```json
{
  "status": "committed",
  "command_id": "cmd_456",
  "committed_at": "2026-05-17T14:30:01Z",
  "events": [
    { "id": "evt_1", "type": "tool_call_requested" },
    { "id": "evt_2", "type": "tool_call_pending_approval" }
  ],
  "result": {
    "tool_call_id": "tc_789",
    "status": "pending_approval",
    "approval_request_id": "ar_111"
  }
}
```

Rejected response:

```json
{
  "status": "rejected",
  "error": {
    "code": "forbidden",
    "message": "actor agent_123 lacks tool.call:shell.run",
    "decision_reason": "no_matching_authority_scope"
  }
}
```

The `result` field shape is command-specific and documented in `03-command-spec.md`. The `events` field is always present on success.

### `POST /v1/command/batch`

For workloads that need to dispatch a sequence atomically (within engine limits). Each item is a full command body. Behavior is **all-or-nothing**: any rejection rolls the whole batch back.

```json
{
  "commands": [
    { "command": "create_session", "actor_id": "agent_1", "workspace_id": "ws", "input": {...} },
    { "command": "append_user_message", "actor_id": "agent_1", "workspace_id": "ws", "input": {...} }
  ]
}
```

Response:

```json
{ "status": "committed", "results": [ {...}, {...} ] }
```

Phase 1 caps batch size at 10. Beyond that, callers issue separate requests and tolerate partial commit.

---

## 5. Subscription API

### `GET /v1/subscribe`  (WebSocket upgrade)

After upgrade, the client sends one or more subscription requests over the connection.

**Open subscription:**

```json
{
  "op": "subscribe",
  "sub_id": "s1",
  "table": "approval_request",
  "filter": {
    "workspace_id": "ws_123",
    "status": "pending"
  }
}
```

**Server response (initial snapshot):**

```json
{
  "sub_id": "s1",
  "type": "snapshot",
  "rows": [ { "id": "ar_111", "status": "pending", ... } ],
  "version": 42
}
```

**Incremental update:**

```json
{
  "sub_id": "s1",
  "type": "upsert",
  "row": { "id": "ar_111", "status": "approved", ... },
  "version": 43
}
```

Delete:

```json
{
  "sub_id": "s1",
  "type": "delete",
  "row_id": "ar_111",
  "version": 44
}
```

**Subscribable tables** (allowlist; expand as needed):

```
session, message, agent_event (filtered),
model_call, tool_call, effect, approval_request,
memory_candidate, memory, memory_use,
context_build, context_item,
workflow_run, workflow_step_run, agent_task,
authority_scope, worker, worker_heartbeat,
replay_run, replay_diff
```

**Filter language.** Phase 1 supports flat equality and `in` against indexed columns. Phase 2 expands to ranges and boolean combinations.

**Backpressure.** Each subscription has a per-client buffer. On overflow the server sends:

```json
{ "sub_id": "s1", "type": "lag", "lost_versions": 10 }
```

…followed by a fresh snapshot. Clients should treat any `lag` as authoritative — they may have missed updates and must re-render.

**Cancel.**

```json
{ "op": "unsubscribe", "sub_id": "s1" }
```

---

## 6. Worker API

### `POST /v1/effects/claim`

See `04-effect-protocol.md` §2.

### `POST /v1/effects/{effect_id}/heartbeat`

### `POST /v1/effects/{effect_id}/start`

### `POST /v1/effects/{effect_id}/observe`

```json
{ "worker_id": "wkr_1", "observation_ref": "art_..." }
```

### Artifact upload

```
POST /v1/artifacts
Content-Type: application/octet-stream
X-Artifact-Kind: tool_output
X-Workspace-Id: ws_123
X-Sensitivity: low

<bytes>
```

Response:

```json
{ "artifact_id": "art_999", "uri": "...", "content_hash": "..." }
```

### Artifact download

```
GET /v1/artifacts/{artifact_id}
```

Returns the bytes with the recorded `Content-Type`. Subject to authority check.

---

## 7. Replay API

### `POST /v1/replay`

```json
{
  "checkpoint_id": "chk_123",
  "mode": "experimental",
  "overrides_ref": "art_..."
}
```

Internally this is `start_replay_run`. Returns:

```json
{
  "replay_run_id": "rr_456",
  "status": "pending"
}
```

### `GET /v1/replay/{replay_run_id}`

Returns the run's current status, summary ref (when complete), and counts of `replay_diff` rows by kind.

### `POST /v1/checkpoints`

```json
{
  "event_id": "evt_999",
  "session_id": "sess_123",
  "workflow_run_id": null,
  "context_build_id": "ctx_456"
}
```

Wraps `create_replay_checkpoint`. Returns `{ "checkpoint_id": "chk_..." }`.

### `GET /v1/checkpoints/{checkpoint_id}`

Returns the checkpoint metadata and the URIs of the four snapshot artifacts.

---

## 8. Health, version, metadata

### `GET /v1/health`

```json
{ "status": "ok", "uptime_s": 1234, "workers_online": 2 }
```

### `GET /v1/version`

```json
{ "actantdb": "0.1.0", "schema": 1, "commit": "..." }
```

### `GET /v1/metadata/commands`

Returns the static catalog of commands the server understands, including each command's input JSON Schema URL. SDK codegen reads this.

### `GET /v1/metadata/tables`

Returns the static catalog of subscribable tables and their column types.

---

## 9. Errors

Errors are always JSON and always carry `status: "rejected"` for commands, or `{"error": {...}}` for non-command endpoints.

```json
{
  "status": "rejected",
  "error": {
    "code": "forbidden",
    "message": "actor ... lacks ...",
    "decision_reason": "no_matching_authority_scope",
    "request_id": "req_..."
  }
}
```

`code` is one of the codes in `03-command-spec.md` §"Standard errors" plus the transport-level codes:

```
unauthenticated         401   missing or invalid token
forbidden               403   policy or authority denied
not_found               404   referenced entity not found
conflict                409   idempotency or version conflict
invalid_input           422   schema validation failed
rate_limited            429   per-actor or per-workspace rate limit
internal_error          500   bug
unavailable             503   server starting or shutting down
```

`request_id` is echoed in server logs; clients should include it in bug reports.

---

## 10. Rate limiting

Per-workspace and per-actor token buckets:

| Bucket             | Default rate         |
| ------------------ | -------------------- |
| `command` per actor| 60/s, burst 120      |
| `command` per ws   | 600/s, burst 1200    |
| `subscribe` per ws | 50 open, 5/s open rate |
| `artifact upload`  | 30/s, 50 MB/s        |

Exceeded buckets return `429 rate_limited` with `Retry-After`. Workers' worker-API endpoints (`claim`, `heartbeat`, `complete_effect`) have their own bucket sized for fleet operation.

---

## Verification

- [ ] Every command in `03-command-spec.md` is invokable via `POST /v1/command`.
- [ ] Every subscribable table in §5 corresponds to a table in `02-data-model.sql`.
- [ ] Every worker-API endpoint here is documented in `04-effect-protocol.md`.
- [ ] Every error code here matches a case in the command pipeline.
- [ ] `GET /v1/metadata/commands` enumerates exactly the set of commands in `03-command-spec.md`.
