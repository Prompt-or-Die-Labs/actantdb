# Work package: `actant-protocol`

## Context

Protocol adapters: MCP (resources/prompts/tools), A2A (agent-to-agent delegation), AP2 (agent payments mandates + intents). Each protocol's interactions become first-class records that enter the chronicle alongside everything else.

## Specs to read first

- `/specs/16-protocols.md` — full file.
- `/specs/13-actant-contract.md` §22 (audit obligation).
- `/agents/actant-worker-mcp.md` (the Phase 2 MCP worker; this crate is the adapter library it depends on).

## Scope

```rust
pub enum Protocol { Mcp, A2a, Ap2 }

#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    fn protocol(&self) -> Protocol;
    async fn ingest(&self, event: IngressEvent) -> Result<Vec<EmittedEvent>, ProtocolError>;
    async fn outbound(&self, action: OutboundAction) -> Result<OutboundResult, ProtocolError>;
}

// MCP
pub struct McpAdapter { /* transports: stdio | sse | websocket */ }
pub async fn mcp_register_server(...) -> Result<McpServerId, ProtocolError>;
pub async fn mcp_pull_resources(...) -> Result<Vec<McpResourceRow>, ProtocolError>;
pub async fn mcp_call_tool(...) -> Result<ToolCallResult, ProtocolError>;

// A2A
pub struct A2aAdapter { /* HMAC/mTLS verifier + outbound client */ }
pub async fn a2a_discover(...) -> Result<A2aCardId, ProtocolError>;
pub async fn a2a_send(...) -> Result<A2aInteractionId, ProtocolError>;

// AP2
pub struct Ap2Adapter { /* never executes payments; records mandates and intents */ }
pub async fn ap2_grant_mandate(...) -> Result<Ap2MandateId, ProtocolError>;
pub async fn ap2_submit_intent(...) -> Result<Ap2IntentId, ProtocolError>;
```

### Feature flags

```
default = ["mcp"]
mcp | a2a | ap2
```

Phase 2 ships MCP. Phase 4 ships A2A. Phase 6 ships AP2.

### Internal modules

```
crates/actant-protocol/src/
├── lib.rs
├── mcp/
│   ├── mod.rs
│   ├── transport/{stdio,sse,websocket}.rs
│   ├── server.rs
│   ├── resource.rs
│   └── prompt.rs
├── a2a/
│   ├── mod.rs
│   ├── card.rs
│   └── interaction.rs
├── ap2/
│   ├── mod.rs
│   ├── mandate.rs
│   └── intent.rs
└── error.rs
```

### Tests

- MCP: registering a server with bad transport rejected; pulling resources from a fixture server produces `mcp_resource` rows.
- A2A: an inbound message without valid signature is recorded with `signature_valid=0` and never produces a delegation.
- AP2: an `ap2_intent` cannot reach `executed` without an `approval_request` in `approved` state.

## Acceptance criteria

- [ ] Build/test/clippy green with `--features mcp` (Phase 2 minimum).
- [ ] Every protocol record cited in `/specs/16-protocols.md` §5 has a CREATE TABLE in migration 0003 and a row-mapper in `actant-storage`.

## Do NOT

- Do NOT execute payments. Recording the mandate and the intent is the entire role.
- Do NOT auto-discover MCP tools without an explicit `register_tool` command.
- Do NOT trust an A2A peer's claims; trust profile gates authority.

## Hand-off

`just ci`.
