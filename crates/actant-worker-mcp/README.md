# actant-worker-mcp

Reference MCP-bridge worker for Phase 2.

Owns:

- MCP client transports: stdio, sse, websocket (via `rmcp`).
- Per-effect tool call: looks up the MCP server URI / command from `tool.kind='mcp'` and the tool's registered config; runs one tool call per lease.
- Maps MCP `Result` / `Error` to `complete_effect`.
- Refuses to expose any MCP tool not registered as an ActantDB `tool` row (no auto-discovery in Phase 2; explicit registration via `register_tool`).
- One tool call per lease — no batching, no implicit reuse.

Binary: `actant-worker-mcp`.

See `agents/actant-worker-mcp.md`.
