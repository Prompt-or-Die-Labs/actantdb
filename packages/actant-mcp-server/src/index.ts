#!/usr/bin/env node
/**
 * @actantdb/mcp-server — Model Context Protocol server exposing the ActantDB
 * ledger to MCP clients (Claude Desktop, Cursor, Continue, etc.).
 *
 * Two transports:
 *
 *   stdio                      `actantdb-mcp --stdio`
 *                              (Claude Desktop and most MCP clients.)
 *
 *   Streamable HTTP            `actantdb-mcp --http --port 8765`
 *                              (HTTP-based MCP clients with optional SSE.)
 *
 * Workspace storage is rooted at `ACTANTDB_STORE_DIR` (default `~/.actantdb`).
 * Each `workspace_id` maps to a SQLite ledger under that directory.
 */

import { createServer, type IncomingMessage, type ServerResponse } from "node:http";

import { McpServer, ResourceTemplate } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { StreamableHTTPServerTransport } from "@modelcontextprotocol/sdk/server/streamableHttp.js";
import { z } from "zod";

import { buildResources, parseActantUri } from "./resources.js";
import { buildTools } from "./tools.js";
import { WorkspaceRegistry } from "./workspaces.js";
import type { Ledger } from "@actantdb/core";

export { buildTools } from "./tools.js";
export { buildResources, parseActantUri } from "./resources.js";
export { evaluatePredicate } from "./predicate.js";
export { WorkspaceRegistry } from "./workspaces.js";
export type { Tools } from "./tools.js";
export type { Resources, ResourceContents } from "./resources.js";

export const SERVER_NAME = "actantdb-mcp";
export const SERVER_VERSION = "0.0.13";

export interface BuildServerOptions {
  /** Root directory for workspace ledgers (default: `~/.actantdb`). */
  storeDir?: string;
  /** Share a single ledger across every workspace (tests / smoke). */
  ledger?: Ledger;
  /** Override server name (mostly for tests). */
  name?: string;
}

/**
 * Build a fully-wired `McpServer` instance with all tools + resources.
 * Caller is responsible for `connect(transport)` and lifecycle management.
 */
export function buildServer(opts: BuildServerOptions = {}): {
  server: McpServer;
  registry: WorkspaceRegistry;
} {
  const registry = new WorkspaceRegistry({
    ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
    ...(opts.ledger !== undefined ? { sharedLedger: opts.ledger } : {}),
  });
  const tools = buildTools(registry);
  const resources = buildResources(registry);

  const server = new McpServer({
    name: opts.name ?? SERVER_NAME,
    version: SERVER_VERSION,
  });

  // --- Tools --------------------------------------------------------------

  server.registerTool(
    "list_runs",
    {
      title: "List runs",
      description: "List recent agent runs in a workspace.",
      inputSchema: {
        workspace_id: z.string().describe("Workspace id (maps to ledger project)."),
        limit: z.number().int().positive().max(1000).optional(),
      },
    },
    async ({ workspace_id, limit }) => {
      const out = await tools.listRuns({ workspace_id, limit });
      return jsonResult(out);
    },
  );

  server.registerTool(
    "get_event",
    {
      title: "Get event",
      description: "Return one event row by id.",
      inputSchema: {
        workspace_id: z.string(),
        event_id: z.string(),
      },
    },
    async ({ workspace_id, event_id }) => {
      const out = await tools.getEvent({ workspace_id, event_id });
      return jsonResult(out);
    },
  );

  server.registerTool(
    "list_events",
    {
      title: "List events",
      description: "List events in a session, paginated by id cursor.",
      inputSchema: {
        workspace_id: z.string(),
        session_id: z.string(),
        limit: z.number().int().positive().max(1000).optional(),
        after: z.string().optional().describe("Cursor: return events with id > after."),
      },
    },
    async ({ workspace_id, session_id, limit, after }) => {
      const args: Parameters<typeof tools.listEvents>[0] = { workspace_id, session_id };
      if (limit !== undefined) args.limit = limit;
      if (after !== undefined) args.after = after;
      const out = await tools.listEvents(args);
      return jsonResult(out);
    },
  );

  server.registerTool(
    "query_predicate",
    {
      title: "Query by predicate",
      description:
        "Return events matching the predicate language (see crates/actant-subscribe/src/predicate.rs). Predicate is a tagged-op tree.",
      inputSchema: {
        workspace_id: z.string(),
        topic: z
          .string()
          .describe("Event kind to filter (`*` for all). Mirrors the subscribe topic."),
        predicate_expr: z.unknown().describe("Predicate AST (JSON; see predicate.rs)."),
        limit: z.number().int().positive().max(1000).optional(),
      },
    },
    async ({ workspace_id, topic, predicate_expr, limit }) => {
      const args: Parameters<typeof tools.queryPredicate>[0] = {
        workspace_id,
        topic,
        predicate_expr,
      };
      if (limit !== undefined) args.limit = limit;
      const out = await tools.queryPredicate(args);
      return jsonResult(out);
    },
  );

  server.registerTool(
    "replay",
    {
      title: "Replay from event",
      description:
        "Replay a run from a checkpoint event. Returns a ReplayDiff describing how the replay diverged from the original.",
      inputSchema: {
        workspace_id: z.string(),
        event_id: z.string(),
        mode: z.enum(["recorded", "tool", "experimental"]).optional(),
        overrides: z
          .object({
            without_memory: z.array(z.string()).optional(),
            model: z.string().nullable().optional(),
            policy: z.string().nullable().optional(),
          })
          .optional(),
        tool_substitutions: z.record(z.string(), z.unknown()).optional(),
        experimental_tool_call_id: z.string().optional(),
        experimental_replacement_result: z.unknown().optional(),
      },
    },
    async (args) => {
      const out = await tools.replay(args);
      return jsonResult(out);
    },
  );

  server.registerTool(
    "list_pending_approvals",
    {
      title: "List pending approvals",
      description:
        "Return approval_required events without a matching approval_decision. Use decide_approval to resolve.",
      inputSchema: { workspace_id: z.string() },
    },
    async ({ workspace_id }) => jsonResult(await tools.listPendingApprovals({ workspace_id })),
  );

  server.registerTool(
    "decide_approval",
    {
      title: "Decide approval",
      description: "Approve, approve-constrained, or deny a pending approval request.",
      inputSchema: {
        workspace_id: z.string(),
        request_id: z.string().describe("The tool_call_id of the pending request."),
        decision: z.enum(["approve", "approve_constrained", "deny"]),
        reason: z.string().optional(),
        approver: z.string().optional(),
        accepted_input: z.unknown().optional(),
        scope: z.string().optional(),
      },
    },
    async (args) => jsonResult(await tools.decideApproval(args)),
  );

  server.registerTool(
    "get_workspace_summary",
    {
      title: "Workspace summary",
      description: "Counts of sessions, events, distinct actors, and pending approvals.",
      inputSchema: { workspace_id: z.string() },
    },
    async ({ workspace_id }) => jsonResult(await tools.getWorkspaceSummary({ workspace_id })),
  );

  // --- Resources ----------------------------------------------------------

  server.registerResource(
    "session",
    new ResourceTemplate("actant://workspace/{ws}/session/{sid}", { list: undefined }),
    {
      title: "Session ledger",
      description: "Live ledger of events for a single session, JSON-serialized.",
      mimeType: "application/json",
    },
    async (uri) => {
      const parsed = parseActantUri(uri.toString());
      if (!parsed || parsed.kind !== "session") {
        throw new Error(`unsupported uri: ${uri.toString()}`);
      }
      return {
        contents: [resources.readSession(parsed.workspaceId, parsed.sessionId)],
      };
    },
  );

  server.registerResource(
    "runs",
    new ResourceTemplate("actant://workspace/{ws}/runs", { list: undefined }),
    {
      title: "Workspace runs",
      description: "List of runs in a workspace with start time + event counts.",
      mimeType: "application/json",
    },
    async (uri) => {
      const parsed = parseActantUri(uri.toString());
      if (!parsed || parsed.kind !== "runs") {
        throw new Error(`unsupported uri: ${uri.toString()}`);
      }
      return { contents: [resources.readRuns(parsed.workspaceId)] };
    },
  );

  return { server, registry };
}

function jsonResult(value: unknown): { content: Array<{ type: "text"; text: string }> } {
  return {
    content: [{ type: "text", text: JSON.stringify(value, null, 2) }],
  };
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

interface CliFlags {
  stdio: boolean;
  http: boolean;
  port: number;
  storeDir: string | undefined;
  help: boolean;
}

function parseCli(argv: string[]): CliFlags {
  const flags: CliFlags = {
    stdio: false,
    http: false,
    port: 8765,
    storeDir: undefined,
    help: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--stdio") flags.stdio = true;
    else if (a === "--http" || a === "--http+sse") flags.http = true;
    else if (a === "--port") {
      const v = argv[++i];
      if (v) flags.port = Number(v);
    } else if (a === "--store-dir") {
      const v = argv[++i];
      if (v) flags.storeDir = v;
    } else if (a === "--help" || a === "-h") flags.help = true;
  }
  if (!flags.stdio && !flags.http) flags.stdio = true; // default to stdio
  return flags;
}

const HELP = `actantdb-mcp — MCP server exposing the ActantDB ledger.

Usage:
  actantdb-mcp [--stdio | --http] [--port 8765] [--store-dir DIR]

Transports:
  --stdio         Spawned by Claude Desktop, Cursor, Continue. (default)
  --http          Streamable HTTP transport (with SSE).

Storage:
  --store-dir     Root for workspace ledgers. Default: $ACTANTDB_STORE_DIR
                  or ~/.actantdb.
`;

async function runStdio(storeDir: string | undefined): Promise<void> {
  const { server } = buildServer({ ...(storeDir !== undefined ? { storeDir } : {}) });
  const transport = new StdioServerTransport();
  await server.connect(transport);
  process.on("SIGINT", async () => {
    await server.close();
    process.exit(0);
  });
}

async function runHttp(port: number, storeDir: string | undefined): Promise<void> {
  const { server } = buildServer({ ...(storeDir !== undefined ? { storeDir } : {}) });
  const transport = new StreamableHTTPServerTransport({ sessionIdGenerator: undefined });
  await server.connect(transport);
  const http = createServer((req: IncomingMessage, res: ServerResponse) => {
    void transport.handleRequest(req, res).catch((err) => {
      // eslint-disable-next-line no-console
      console.error("[actantdb-mcp] http request failed:", err);
      try {
        res.statusCode = 500;
        res.end("internal error");
      } catch {
        // socket already closed
      }
    });
  });
  http.listen(port, () => {
    // eslint-disable-next-line no-console
    console.error(`[actantdb-mcp] http listening on :${port}`);
  });
  process.on("SIGINT", async () => {
    http.close();
    await server.close();
    process.exit(0);
  });
}

// Detect direct execution. Works for `node dist/index.js` and `actantdb-mcp`.
const invokedAsBin =
  typeof process !== "undefined" &&
  Array.isArray(process.argv) &&
  process.argv[1] !== undefined &&
  /actantdb-mcp$|actant-mcp-server[\\/]dist[\\/]index\.js$/.test(process.argv[1]);

if (invokedAsBin || process.env["ACTANTDB_MCP_FORCE_RUN"] === "1") {
  const flags = parseCli(process.argv.slice(2));
  if (flags.help) {
    process.stdout.write(HELP);
    process.exit(0);
  }
  const storeDir = flags.storeDir ?? process.env["ACTANTDB_STORE_DIR"];
  const start = flags.http ? runHttp(flags.port, storeDir) : runStdio(storeDir);
  start.catch((err) => {
    // eslint-disable-next-line no-console
    console.error("[actantdb-mcp] failed to start:", err);
    process.exit(1);
  });
}
