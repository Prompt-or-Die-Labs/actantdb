# 10 — Build your first MCP tool on top of ActantDB

`@actantdb/mcp-server` already exposes the ledger to MCP clients. This
recipe shows how to **extend** that surface with custom tools so Claude
Desktop (or Cursor, or any MCP client) can run domain-specific actions
against your own data — gated by the same Guard policy that protects
your agent.

## Why do this

- A teammate uses Claude Desktop to triage failures. You give them an
  MCP tool `summarize_failures(date)` that runs against your live
  ledger.
- You want Claude to draft a refund decision but a human to approve it.
  Add a `draft_refund` tool that emits an `approval_required` event;
  any approver — your existing Studio UI or a different MCP client —
  resolves it.

## Scaffold

```bash
mkdir my-actant-mcp && cd my-actant-mcp
npm init -y
npm install @actantdb/core @actantdb/mcp-server @modelcontextprotocol/sdk zod
```

## Extend the built-in server

```ts
// src/server.ts
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { buildServer } from "@actantdb/mcp-server";
import { z } from "zod";

const { server, registry } = buildServer({
  storeDir: process.env.ACTANTDB_STORE_DIR ?? "/var/lib/actantdb",
});

// Add your domain-specific tool on top of the ledger primitives.
server.registerTool(
  "summarize_failures",
  {
    title: "Summarize failed tool calls",
    description: "Return tool calls in a workspace that completed with status=error.",
    inputSchema: {
      workspace_id: z.string(),
      since: z.string().optional().describe("ISO timestamp lower bound."),
    },
  },
  async ({ workspace_id, since }) => {
    const ledger = registry.get(workspace_id);
    const events = ledger
      .query({ kind: "tool_call_completed" })
      .filter((e) => (e.payload as { status?: string }).status === "error")
      .filter((e) => !since || e.created_at >= since);
    return {
      content: [
        {
          type: "text",
          text: JSON.stringify(
            {
              workspace_id,
              count: events.length,
              failures: events.map((e) => ({
                id: e.id,
                run_id: e.run_id,
                created_at: e.created_at,
                payload: e.payload,
              })),
            },
            null,
            2,
          ),
        },
      ],
    };
  },
);

await server.connect(new StdioServerTransport());
```

Build with `tsc` and ship the resulting `dist/server.js`.

## Add it to Claude Desktop

```json
{
  "mcpServers": {
    "my-actant": {
      "command": "node",
      "args": ["/abs/path/to/my-actant-mcp/dist/server.js"],
      "env": { "ACTANTDB_STORE_DIR": "/var/lib/actantdb" }
    }
  }
}
```

Restart Claude Desktop. Open a chat and ask "What failed in workspace
support yesterday?" — Claude picks `summarize_failures`, fills in the
args, and renders the JSON result.

## Tool that emits an approval

```ts
import { ulid } from "@actantdb/core";

server.registerTool(
  "draft_refund",
  {
    title: "Draft a refund (requires human approval)",
    description:
      "Records an approval_required event. The refund does NOT fire until a human (via Studio or another MCP client) decides.",
    inputSchema: {
      workspace_id: z.string(),
      invoice: z.string(),
      amount_cents: z.number().int().positive(),
      reason: z.string(),
    },
  },
  async ({ workspace_id, invoice, amount_cents, reason }) => {
    const ledger = registry.get(workspace_id);
    const tcid = `tc-${ulid()}`;
    const runId = `mcp-${ulid()}`;
    ledger.append({
      kind: "agent_run_started",
      runId,
      payload: { project: workspace_id, meta: { source: "mcp:draft_refund" } },
      sensitivity: "low",
    });
    ledger.append({
      kind: "tool_call_requested",
      runId,
      payload: { tool_call_id: tcid, tool: "issue_refund", risk: "high", args: { invoice, amount_cents } },
      sensitivity: "low",
    });
    ledger.append({
      kind: "approval_required",
      runId,
      payload: { tool_call_id: tcid, tool: "issue_refund", reason, args: { invoice, amount_cents } },
      sensitivity: "low",
    });
    return {
      content: [
        {
          type: "text",
          text: JSON.stringify(
            { status: "awaiting_approval", request_id: tcid, run_id: runId },
            null,
            2,
          ),
        },
      ],
    };
  },
);
```

Now Claude can *draft* the refund — but the refund only happens when a
human resolves it via `decide_approval` (already exposed by
`@actantdb/mcp-server`), or via your Studio approval queue.

## Test it

`@actantdb/testing` works against any ledger, including the registry's:

```ts
import { describe, it } from "vitest";
import { createTestLedger, expectEventEmitted } from "@actantdb/testing";
// ... call your tool with the test ledger plumbed in ...
expectEventEmitted(t, "approval_required", { tool: "issue_refund" });
```

## Why this matters

Every MCP tool you add inherits ActantDB's:

- **Hash-chained audit trail** — every action a model takes through your
  tool is on the ledger, immutable, replayable.
- **Approval gating** — no extra plumbing required to require human-in-
  the-loop for risky tools.
- **Sensitivity ceiling** — Guard refuses to act on capsule-bound,
  secret content even if Claude asks nicely.

## See also

- [Recipe 03](./03-wire-into-nextjs.md) — same server, but exposed over HTTP.
- [`@actantdb/mcp-server` README](../../packages/actant-mcp-server/README.md) — full tool/resource reference.
