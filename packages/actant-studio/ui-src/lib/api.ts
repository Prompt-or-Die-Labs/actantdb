// Typed fetch helpers against the Node Studio HTTP API.
// Server routes — see ../../src/server.ts:
//   GET  /api/info
//   GET  /api/events?run=<runId>
//   GET  /api/approvals
//   POST /api/approvals/decide   { toolCallId, decision }
//   POST /api/replay             { eventId, overrides?, useStrictPolicy?, mode? }
//
// Types come from @actantdb/types where available; we narrow to the
// shapes the server actually returns.

import type {
  ActantEvent,
  ApprovalDecision,
  ApprovalRequest,
  ReplayDiff,
  ReplayOverrides,
  ReplayRun,
} from "@actantdb/types";

// `ApprovalRecord` lives in @actantdb/core; we don't depend on core from
// the UI bundle, so mirror the shape locally. The server hands these back
// over JSON via /api/approvals.
export interface ApprovalRecord {
  toolCallId: string;
  runId: string;
  status: "pending" | "approved" | "approved_constrained" | "denied";
  request: ApprovalRequest;
  decision?: ApprovalDecision;
  createdAt: string;
  decidedAt?: string;
}

export interface RunSummary {
  runId: string;
  events: number;
  startedAt: string;
}

export interface StudioInfo {
  project: string;
  dbPath: string;
  runs: RunSummary[];
}

export interface ReplayResponse {
  replay: ReplayRun;
  diff: ReplayDiff;
}

export type ReplayMode = "recorded" | "model" | "policy" | "memory";

export interface ReplayRequestBody {
  eventId: string;
  overrides?: ReplayOverrides;
  useStrictPolicy?: boolean;
  mode?: ReplayMode;
}

async function getJSON<T>(url: string): Promise<T> {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return (await r.json()) as T;
}

async function postJSON<T>(url: string, body: unknown): Promise<T> {
  const r = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return (await r.json()) as T;
}

export const api = {
  info(): Promise<StudioInfo> {
    return getJSON<StudioInfo>("/api/info");
  },
  events(runId?: string): Promise<{ events: ActantEvent[] }> {
    const suffix = runId ? `?run=${encodeURIComponent(runId)}` : "";
    return getJSON<{ events: ActantEvent[] }>(`/api/events${suffix}`);
  },
  approvals(): Promise<{ approvals: ApprovalRecord[] }> {
    return getJSON<{ approvals: ApprovalRecord[] }>("/api/approvals");
  },
  decide(toolCallId: string, decision: ApprovalDecision): Promise<unknown> {
    return postJSON("/api/approvals/decide", { toolCallId, decision });
  },
  replay(body: ReplayRequestBody): Promise<ReplayResponse> {
    return postJSON<ReplayResponse>("/api/replay", body);
  },
};
