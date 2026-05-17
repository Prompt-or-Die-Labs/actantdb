# Work package: `actant-worker-mcp`

## Context

Bridge to MCP servers. Phase 2 supports stdio, SSE, and websocket MCP transports. One tool call per lease — no batching, no auto-discovery.

## Specs to read first

- `/specs/04-effect-protocol.md` §7 (`tool.call` for kind=mcp).
- `/specs/03-command-spec.md` §`register_tool` — MCP tools must be registered explicitly before they can be called.
- MCP spec (https://modelcontextprotocol.io) for transport and message shapes.

## Scope

### Behavior

- On boot, register with capabilities `["tool.call"]` and a list of `mcp_kinds` it supports (`stdio`, `sse`, `websocket`).
- On each lease for a `tool.call` whose tool's `kind='mcp'`, look up the registered MCP server config (transport + URI / command + auth) from the tool row.
- Open the transport for the duration of the lease only (no persistent session in Phase 2; persistent sessions arrive in Phase 4 when MCP sampling is wired).
- Send a single tool-call request; collect the result.
- Build an observation with `evidence_type='tool_output'` and the MCP result content as the artifact.
- Call `complete`.

### Internal modules

```
crates/actant-worker-mcp/src/
├── main.rs
├── lib.rs
├── transport/
│   ├── mod.rs
│   ├── stdio.rs
│   ├── sse.rs
│   └── websocket.rs
├── client.rs
└── registry.rs       // resolve tool_name → MCP server config
```

### Tests

- A stdio MCP echo server round-trips a tool call.
- An unregistered tool name (not in `tool` table) returns `not_found`.
- A tool whose registered transport doesn't match the worker's capability returns `precondition_failed` without opening the transport.

## Acceptance criteria

- [ ] Build / test / clippy green.
- [ ] CI runs a smoke test against a bundled "echo" MCP server.
- [ ] No persistent state between leases (verified by an integration test that creates two leases in sequence).

## Do NOT

- Do NOT discover MCP tools at runtime. Every callable tool must have a registered `tool` row.
- Do NOT keep MCP sessions alive across leases in Phase 2.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
