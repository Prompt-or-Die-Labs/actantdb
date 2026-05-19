# @actantdb/mcp-server

Model Context Protocol server that exposes the ActantDB ledger to MCP clients
(Claude Desktop, Cursor, Continue, Cline, anywhere MCP is supported).

Two transports ship in the same binary:

| Transport | Use it for |
|-----------|------------|
| `stdio`   | Claude Desktop, Cursor, Continue, most local MCP clients (default). |
| `http`    | Streamable HTTP (with SSE upgrade) for hosted or remote MCP clients. |

## Install

```bash
npm install -g @actantdb/mcp-server
# or run on demand:
npx -y @actantdb/mcp-server --stdio
```

The binary name is `actantdb-mcp`.

## Add to Claude Desktop

Edit `~/Library/Application Support/Claude/claude_desktop_config.json`
(macOS) — Windows: `%APPDATA%\Claude\claude_desktop_config.json` — and add:

```json
{
  "mcpServers": {
    "actantdb": {
      "command": "npx",
      "args": ["-y", "@actantdb/mcp-server", "--stdio"],
      "env": {
        "ACTANTDB_STORE_DIR": "/Users/you/.actantdb"
      }
    }
  }
}
```

Restart Claude Desktop. The server's tools (listed below) show up in the
tool picker.

## Add to Cursor

Cursor reads MCP config from `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "actantdb": {
      "command": "npx",
      "args": ["-y", "@actantdb/mcp-server", "--stdio"]
    }
  }
}
```

## Add to Continue

`~/.continue/config.json`:

```json
{
  "mcpServers": {
    "actantdb": {
      "transport": { "type": "stdio", "command": "npx", "args": ["-y", "@actantdb/mcp-server"] }
    }
  }
}
```

## Run over HTTP

```bash
actantdb-mcp --http --port 8765 --store-dir ~/.actantdb
```

Point any Streamable HTTP MCP client at `http://localhost:8765/`. The
transport honors the standard MCP session-id header negotiation.

## Tools

The server addresses ledger data by `workspace_id` (mapped to a SQLite ledger
project) and `session_id` (mapped to a run id).

| Tool                        | Input                                                                | Returns |
|-----------------------------|----------------------------------------------------------------------|---------|
| `list_runs`                 | `workspace_id`, `limit?`                                             | Recent runs with start time + event counts. |
| `get_event`                 | `workspace_id`, `event_id`                                           | One event row. |
| `list_events`               | `workspace_id`, `session_id`, `limit?`, `after?`                     | Events in a session, paginated by id cursor. |
| `query_predicate`           | `workspace_id`, `topic`, `predicate_expr`, `limit?`                  | Events matching the predicate AST (mirrors `crates/actant-subscribe/src/predicate.rs`). |
| `replay`                    | `workspace_id`, `event_id`, `mode?`, `overrides?`, `tool_substitutions?`, `experimental_*?` | `ReplayDiff` from `@actantdb/replay`. |
| `list_pending_approvals`    | `workspace_id`                                                       | `approval_required` events without a matching decision. |
| `decide_approval`           | `workspace_id`, `request_id`, `decision`, `reason?`, `approver?`, `accepted_input?`, `scope?` | Records an `approval_decision`. |
| `get_workspace_summary`     | `workspace_id`                                                       | Counts of sessions, events, actors, and pending approvals. |

### `predicate_expr` shape

Predicates are tagged JSON objects. See
`crates/actant-subscribe/src/predicate.rs` for the canonical grammar; the TS
evaluator is a 1:1 port of those semantics.

```json
{
  "op": "and",
  "args": [
    { "op": "eq", "field": "kind", "value": "tool_call_completed" },
    { "op": "eq", "field": "payload.status", "value": "ok" }
  ]
}
```

Comparators (`eq`, `ne`, `lt`, `le`, `gt`, `ge`) take `field` (dotted path)
and `value`. `exists` takes only `field`. Logical ops: `and`/`or` take
`args` arrays, `not` takes `arg`. `true`/`false` are constant predicates.

## Resources

| URI template                                          | Returns |
|-------------------------------------------------------|---------|
| `actant://workspace/{ws}/session/{sid}`               | JSON: every event in the session. |
| `actant://workspace/{ws}/runs`                        | JSON: list of runs in the workspace. |

## Programmatic use

```ts
import { buildServer } from "@actantdb/mcp-server";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

const { server } = buildServer({ storeDir: "/var/lib/actantdb" });
await server.connect(new StdioServerTransport());
```

`buildServer` returns the underlying `McpServer` instance plus the workspace
registry, so you can extend it with additional tools or share a ledger with
in-process consumers.

## Environment

| Variable | Default | Effect |
|----------|---------|--------|
| `ACTANTDB_STORE_DIR` | `~/.actantdb` | Root directory for workspace ledgers. |
