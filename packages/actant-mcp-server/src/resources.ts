/**
 * Resource implementations for the ActantDB MCP server.
 *
 *   actant://workspace/{ws}/session/{sid}    live ledger for one session
 *   actant://workspace/{ws}/runs             list of runs for the workspace
 *
 * Returned as `application/json` text blobs — clients (Claude Desktop, Cursor)
 * render them in their inspector panels.
 */

import type { WorkspaceRegistry } from "./workspaces.js";

export interface ResourceContents {
  uri: string;
  mimeType: string;
  text: string;
}

export interface Resources {
  readSession(workspaceId: string, sessionId: string): ResourceContents;
  readRuns(workspaceId: string): ResourceContents;
}

export function buildResources(registry: WorkspaceRegistry): Resources {
  return {
    readSession(workspaceId, sessionId) {
      const ledger = registry.get(workspaceId);
      const events = ledger.query({ runId: sessionId, limit: 1000 });
      return {
        uri: `actant://workspace/${workspaceId}/session/${sessionId}`,
        mimeType: "application/json",
        text: JSON.stringify({ workspace_id: workspaceId, session_id: sessionId, events }, null, 2),
      };
    },
    readRuns(workspaceId) {
      const ledger = registry.get(workspaceId);
      const all = ledger.query({ limit: 10_000 });
      const byRun = new Map<string, { started_at: string; event_count: number }>();
      for (const e of all) {
        const cur = byRun.get(e.run_id);
        if (cur) {
          cur.event_count += 1;
          if (e.created_at < cur.started_at) cur.started_at = e.created_at;
        } else {
          byRun.set(e.run_id, { started_at: e.created_at, event_count: 1 });
        }
      }
      const runs = [...byRun.entries()]
        .map(([run_id, v]) => ({ run_id, ...v }))
        .sort((a, b) => (a.started_at < b.started_at ? 1 : -1));
      return {
        uri: `actant://workspace/${workspaceId}/runs`,
        mimeType: "application/json",
        text: JSON.stringify({ workspace_id: workspaceId, runs }, null, 2),
      };
    },
  };
}

/**
 * Parse an `actant://workspace/{ws}/...` URI. Returns null on mismatch.
 * Pure helper; safe to use without instantiating the server.
 */
export function parseActantUri(
  uri: string,
):
  | { kind: "session"; workspaceId: string; sessionId: string }
  | { kind: "runs"; workspaceId: string }
  | null {
  if (!uri.startsWith("actant://workspace/")) return null;
  const rest = uri.slice("actant://workspace/".length);
  const parts = rest.split("/");
  if (parts.length === 2 && parts[1] === "runs" && parts[0]) {
    return { kind: "runs", workspaceId: parts[0] };
  }
  if (parts.length === 4 && parts[1] === "session" && parts[0] && parts[2] && parts[3] === "") {
    // trailing slash variant — handle but accept the cleaner 3-part form below.
    return { kind: "session", workspaceId: parts[0], sessionId: parts[2] };
  }
  if (parts.length === 3 && parts[1] === "session" && parts[0] && parts[2]) {
    return { kind: "session", workspaceId: parts[0], sessionId: parts[2] };
  }
  return null;
}
