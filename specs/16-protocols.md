# 16 — Protocols (MCP, A2A, AP2)

ActantDB makes protocol interactions first-class. MCP, A2A, and AP2 records enter the chronicle alongside model calls, tool calls, and effects. Adapters live in `actant-protocol`.

## Why this matters

Agents in 2026 increasingly interoperate. An MCP server exposes resources / prompts / tools; an A2A peer accepts task delegation; an AP2 mandate authorizes spend. None of these can live as untyped JSON in some log file — every interaction needs the same governance, replay, and audit guarantees as everything else.

## 1. MCP — Model Context Protocol

### Records

```sql
mcp_server (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL,
    name            TEXT NOT NULL,
    transport       TEXT NOT NULL,    -- 'stdio'|'sse'|'websocket'
    uri             TEXT NOT NULL,
    auth_ref        TEXT,             -- secret_ref
    capabilities    TEXT NOT NULL,    -- JSON: { resources, prompts, tools, sampling }
    registered_at   TEXT NOT NULL,
    retired_at      TEXT
);

mcp_resource (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL,
    server_id       TEXT NOT NULL,
    uri             TEXT NOT NULL,    -- as defined by MCP
    name            TEXT,
    mime_type       TEXT,
    sensitivity     TEXT NOT NULL,
    capsule_id      TEXT,
    last_read_at    TEXT,
    UNIQUE (server_id, uri)
);

mcp_prompt (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL,
    server_id       TEXT NOT NULL,
    name            TEXT NOT NULL,
    schema_ref      TEXT NOT NULL,
    version         INTEGER NOT NULL,
    UNIQUE (server_id, name, version)
);

-- MCP tools register as ordinary `tool` rows with kind='mcp' + a foreign key
-- to mcp_server. The tool_call effect_type for MCP is 'tool.call' as usual.
```

### Behaviors

- **Resource indexing.** Subscribed MCP resources are pulled by `actant-ingress`, normalized, and submitted to `actant-index` like any other indexable object. Sensitivity defaults to the MCP server's declared sensitivity; capsules can override.
- **Prompt versioning.** Every MCP prompt sent into a model call records `(server_id, name, version)` for replay.
- **Tool registration.** `actant mcp wrap <tool>` mints a Tool row with `kind='mcp'`, a tool schema version, and a default approval policy. The MCP tool then goes through the standard tool-call lifecycle.

### CLI

```
actant mcp add <name> --transport stdio --command "..."
actant mcp add <name> --transport sse --url "..."
actant mcp import <file|url>
actant mcp list
actant mcp tools <server>
actant mcp resources <server>
actant mcp wrap <tool> [--approval required|low|medium|high]
```

## 2. A2A — Agent-to-Agent

### Records

```sql
a2a_card (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL,
    peer_name       TEXT NOT NULL,
    endpoint        TEXT NOT NULL,
    capabilities    TEXT NOT NULL,    -- JSON: skills, modalities, cost profile
    auth_ref        TEXT,
    trust_ref       TEXT,              -- trust_profile.id of the peer (if known)
    discovered_at   TEXT NOT NULL,
    retired_at      TEXT
);

a2a_interaction (
    id                  TEXT PRIMARY KEY,
    workspace_id        TEXT NOT NULL,
    peer_card_id        TEXT NOT NULL,
    direction           TEXT NOT NULL,   -- 'outbound'|'inbound'
    task_id             TEXT NOT NULL,
    intent_id           TEXT,            -- our intent if outbound
    delegation_id       TEXT,            -- our delegation if outbound
    state               TEXT NOT NULL,   -- 'open'|'awaiting'|'completed'|'failed'
    payload_ref         TEXT,
    signature           TEXT,
    created_at          TEXT NOT NULL,
    finished_at         TEXT
);
```

### Behaviors

- A2A peers do not get implicit authority. A delegation row + an `authority_scope` bound to the peer's actor must exist before any outbound message.
- Inbound A2A messages enter via `actant-ingress` with HMAC / mTLS verification, hit `actant-policy` for authorization, and produce an `intent` row that the local agent fulfills.
- A peer's behavior feeds into `trust_profile` — repeated failed-delegation events lower its score.

### CLI

```
actant a2a peers
actant a2a discover <endpoint>
actant a2a interactions [--peer ...] [--state ...]
actant a2a delegate <peer> <goal> --budget <usd> --deadline <iso>
```

## 3. AP2 — Agent Payments Protocol

ActantDB does **not** process payments. It records the **mandate** and the **audit trail**; payment execution always goes through an external processor and always requires explicit approval.

### Records

```sql
ap2_mandate (
    id                          TEXT PRIMARY KEY,
    workspace_id                TEXT NOT NULL,
    granted_by_actor_id         TEXT NOT NULL,        -- human user
    holder_actor_id             TEXT NOT NULL,        -- agent receiving the mandate
    purpose                     TEXT NOT NULL,
    spend_limit_usd             REAL NOT NULL,
    spend_used_usd              REAL NOT NULL DEFAULT 0,
    cryptographic_proof_ref     TEXT NOT NULL,        -- signed JWT / similar
    expires_at                  TEXT NOT NULL,
    revoked_at                  TEXT,
    created_at                  TEXT NOT NULL
);

ap2_intent (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL,
    mandate_id      TEXT NOT NULL,
    purpose         TEXT NOT NULL,
    amount_usd      REAL NOT NULL,
    payee           TEXT NOT NULL,
    signed_payload  TEXT NOT NULL,
    status          TEXT NOT NULL,    -- 'pending_approval'|'approved'|'denied'|'executed'|'failed'
    created_at      TEXT NOT NULL,
    decided_at      TEXT
);

ap2_transaction (
    id              TEXT PRIMARY KEY,
    intent_id       TEXT NOT NULL,
    processor       TEXT NOT NULL,    -- 'stripe'|'wise'|...
    external_ref    TEXT NOT NULL,
    amount_usd      REAL NOT NULL,
    status          TEXT NOT NULL,
    created_at      TEXT NOT NULL
);
```

### Behaviors

- Every `ap2_intent` produces an `approval_request` with `risk_level='critical'`. Auto-approval is forbidden unless the workspace explicitly allows mandate-bounded auto-spend below a threshold.
- Mandates are revocable; revocation cascades: pending intents under that mandate go to `denied`.
- Replay can reconstruct the cryptographic proof chain to demonstrate verifiable intent for audit.

### CLI

```
actant ap2 mandate list
actant ap2 mandate grant <agent> --purpose ... --limit 50 --expires 2026-12-31
actant ap2 mandate revoke <id>
actant ap2 intents [--status ...]
actant ap2 audit <mandate-id> [--export ...]
```

## 4. Common adapter

All three protocols share an adapter pattern in `actant-protocol`:

```rust
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    fn protocol(&self) -> Protocol;        // Mcp | A2A | Ap2
    async fn ingest(&self, event: IngressEvent) -> Result<Vec<EmittedEvent>, ProtocolError>;
    async fn outbound(&self, action: OutboundAction) -> Result<OutboundResult, ProtocolError>;
}
```

Adapters live behind feature flags; Phase 2 ships MCP, Phase 4 ships A2A, Phase 6 ships AP2.

## 5. Invariants

1. No protocol record bypasses approval where the protocol's risk requires it. AP2 critical risk auto-approval is forbidden by default; MCP tool calls follow the same approval flow as ordinary tools; A2A delegations follow the standard delegation flow.
2. Every protocol interaction has a chronicle event with the protocol name as the `causality_kind` payload prefix.
3. Sensitivity carries from MCP resources / A2A payloads / AP2 mandates into the index as capsule policy.

## Verification

- [ ] Every table in §1–§3 has a CREATE TABLE in `/migrations/0003_ai_native_and_reliability.sql`.
- [ ] `actant mcp wrap` produces a `tool` row whose `tool_call` lifecycle is identical to a native tool.
- [ ] A2A authority is structurally bounded by a `delegation` row.
- [ ] An AP2 intent without an `approval_request` cannot transition to `executed`.
