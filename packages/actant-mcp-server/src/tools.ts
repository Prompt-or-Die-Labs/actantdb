/**
 * Tool implementations for the ActantDB MCP server.
 *
 * Every tool takes a parsed argument object and returns a JSON-serializable
 * result. They are wired into the MCP `registerTool` surface in `index.ts`.
 *
 * All event/run/workspace ids stay opaque strings — the server is a pass-
 * through layer onto `@actantdb/core` + `@actantdb/replay`. No new domain
 * types live here.
 */

import { runFromEvent } from "@actantdb/replay";
import type { ActantEvent, ApprovalDecision, ReplayDiff } from "@actantdb/types";

import { evaluatePredicate } from "./predicate.js";
import type { WorkspaceRegistry } from "./workspaces.js";

/** Maximum events returned by any single list_* call. */
export const DEFAULT_LIMIT = 100;
export const MAX_LIMIT = 1000;

function clampLimit(limit: number | undefined): number {
  if (limit === undefined) return DEFAULT_LIMIT;
  if (!Number.isFinite(limit) || limit <= 0) return DEFAULT_LIMIT;
  return Math.min(Math.floor(limit), MAX_LIMIT);
}

export interface Tools {
  listRuns: (args: {
    workspace_id: string;
    limit?: number;
  }) => Promise<{ runs: Array<{ run_id: string; started_at: string; event_count: number }> }>;

  getEvent: (args: {
    workspace_id: string;
    event_id: string;
  }) => Promise<{ event: ActantEvent | null }>;

  listEvents: (args: {
    workspace_id: string;
    session_id: string;
    limit?: number;
    after?: string;
  }) => Promise<{ events: ActantEvent[]; next_after: string | null }>;

  queryPredicate: (args: {
    workspace_id: string;
    topic: string;
    predicate_expr: unknown;
    limit?: number;
  }) => Promise<{ matches: ActantEvent[] }>;

  replay: (args: {
    workspace_id: string;
    event_id: string;
    mode?: "recorded" | "tool" | "experimental";
    overrides?: { without_memory?: string[]; model?: string | null; policy?: string | null };
    tool_substitutions?: Record<string, unknown>;
    experimental_tool_call_id?: string;
    experimental_replacement_result?: unknown;
  }) => Promise<{ diff: ReplayDiff; replay_event_count: number }>;

  listPendingApprovals: (args: {
    workspace_id: string;
  }) => Promise<{
    pending: Array<{ tool_call_id: string; run_id: string; tool: string; reason: string; created_at: string }>;
  }>;

  decideApproval: (args: {
    workspace_id: string;
    request_id: string;
    decision: "approve" | "approve_constrained" | "deny";
    reason?: string;
    approver?: string;
    accepted_input?: unknown;
    scope?: string;
  }) => Promise<{ status: string; tool_call_id: string }>;

  getWorkspaceSummary: (args: {
    workspace_id: string;
  }) => Promise<{
    workspace_id: string;
    session_count: number;
    event_count: number;
    actor_count: number;
    pending_approvals: number;
  }>;
}

export function buildTools(registry: WorkspaceRegistry): Tools {
  return {
    async listRuns({ workspace_id, limit }) {
      const ledger = registry.get(workspace_id);
      const all = ledger.query({ limit: clampLimit(limit) * 50 });
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
        .sort((a, b) => (a.started_at < b.started_at ? 1 : -1))
        .slice(0, clampLimit(limit));
      return { runs };
    },

    async getEvent({ workspace_id, event_id }) {
      const ledger = registry.get(workspace_id);
      const ev = ledger.get(event_id);
      return { event: ev ?? null };
    },

    async listEvents({ workspace_id, session_id, limit, after }) {
      const ledger = registry.get(workspace_id);
      const filter: { runId: string; limit: number; sinceId?: string } = {
        runId: session_id,
        limit: clampLimit(limit),
      };
      if (after !== undefined) filter.sinceId = after;
      const events = ledger.query(filter);
      const last = events[events.length - 1];
      return {
        events,
        next_after: events.length >= clampLimit(limit) && last ? last.id : null,
      };
    },

    async queryPredicate({ workspace_id, topic, predicate_expr, limit }) {
      const ledger = registry.get(workspace_id);
      const kindFilter = topic === "*" ? undefined : (topic as ActantEvent["kind"]);
      const events = ledger.query({
        ...(kindFilter ? { kind: kindFilter } : {}),
        limit: clampLimit(limit) * 10,
      });
      const matches: ActantEvent[] = [];
      for (const e of events) {
        const root = { payload: e.payload, kind: e.kind, run_id: e.run_id };
        if (evaluatePredicate(predicate_expr, JSON.parse(JSON.stringify(root)))) {
          matches.push(e);
          if (matches.length >= clampLimit(limit)) break;
        }
      }
      return { matches };
    },

    async replay({
      workspace_id,
      event_id,
      mode,
      overrides,
      tool_substitutions,
      experimental_tool_call_id,
      experimental_replacement_result,
    }) {
      const ledger = registry.get(workspace_id);
      const opts: Parameters<typeof runFromEvent>[0] = {
        ledger,
        eventId: event_id,
        overrides: overrides ?? {},
      };
      if (mode) opts.mode = mode;
      if (tool_substitutions) opts.toolSubstitutions = tool_substitutions;
      if (experimental_tool_call_id !== undefined)
        opts.experimentalToolCallId = experimental_tool_call_id;
      if (experimental_replacement_result !== undefined)
        opts.experimentalReplacementResult = experimental_replacement_result;
      const replay = runFromEvent(opts);
      // Build the diff inline to avoid an extra import path.
      const original = ledger.query({ runId: replay.original_run });
      const diff = diffArrays(original, replay.events);
      return { diff, replay_event_count: replay.events.length };
    },

    async listPendingApprovals({ workspace_id }) {
      const ledger = registry.get(workspace_id);
      const events = ledger.query({ kind: "approval_required" });
      // Filter out any approval_required that already has a matching decision.
      const decisions = ledger.query({ kind: "approval_decision" });
      const decided = new Set(
        decisions
          .map((e) => (e.payload as { tool_call_id?: string } | undefined)?.tool_call_id)
          .filter(Boolean) as string[],
      );
      const pending = events
        .map((e) => {
          const p = e.payload as { tool_call_id?: string; tool?: string; reason?: string };
          return {
            tool_call_id: p.tool_call_id ?? "",
            run_id: e.run_id,
            tool: p.tool ?? "",
            reason: p.reason ?? "",
            created_at: e.created_at,
          };
        })
        .filter((p) => p.tool_call_id && !decided.has(p.tool_call_id));
      return { pending };
    },

    async decideApproval({
      workspace_id,
      request_id,
      decision,
      reason,
      approver,
      accepted_input,
      scope,
    }) {
      const ledger = registry.get(workspace_id);
      const reqEvent = ledger
        .query({ kind: "approval_required" })
        .find(
          (e) => (e.payload as { tool_call_id?: string } | undefined)?.tool_call_id === request_id,
        );
      if (!reqEvent) {
        throw new Error(`pending approval not found: ${request_id}`);
      }
      const dec = buildDecision(decision, { reason, approver, accepted_input, scope });
      ledger.append({
        kind: "approval_decision",
        runId: reqEvent.run_id,
        payload: { tool_call_id: request_id, ...dec },
        sensitivity: "low",
      });
      return { status: "recorded", tool_call_id: request_id };
    },

    async getWorkspaceSummary({ workspace_id }) {
      const ledger = registry.get(workspace_id);
      const all = ledger.query({ limit: MAX_LIMIT * 10 });
      const sessions = new Set<string>();
      const actors = new Set<string>();
      let pending = 0;
      const decided = new Set<string>();
      for (const e of all) {
        sessions.add(e.run_id);
        if (e.kind === "approval_decision") {
          const p = e.payload as { tool_call_id?: string };
          if (p.tool_call_id) decided.add(p.tool_call_id);
        }
        if (e.kind === "approval_decision") {
          const p = e.payload as { approver?: string };
          if (p.approver) actors.add(p.approver);
        }
      }
      for (const e of all) {
        if (e.kind === "approval_required") {
          const p = e.payload as { tool_call_id?: string };
          if (p.tool_call_id && !decided.has(p.tool_call_id)) pending += 1;
        }
      }
      return {
        workspace_id,
        session_count: sessions.size,
        event_count: all.length,
        actor_count: actors.size,
        pending_approvals: pending,
      };
    },
  };
}

function buildDecision(
  kind: "approve" | "approve_constrained" | "deny",
  extras: { reason?: string; approver?: string; accepted_input?: unknown; scope?: string },
): ApprovalDecision {
  const approver = extras.approver ?? "mcp-client";
  if (kind === "approve") {
    return { decision: "approve", approver, scope: extras.scope ?? "single-call" };
  }
  if (kind === "approve_constrained") {
    return {
      decision: "approve_constrained",
      approver,
      scope: extras.scope ?? "single-call",
      accepted_input: extras.accepted_input ?? null,
    };
  }
  return { decision: "deny", approver, reason: extras.reason ?? "denied via mcp" };
}

function diffArrays(a: ActantEvent[], b: ActantEvent[]): ReplayDiff {
  const entries: ReplayDiff["entries"] = [];
  const len = Math.max(a.length, b.length);
  for (let i = 0; i < len; i++) {
    const x = a[i];
    const y = b[i];
    if (x && !y) {
      entries.push({ kind: x.kind, diff: "missing", a: x.payload });
      continue;
    }
    if (!x && y) {
      entries.push({ kind: y.kind, diff: "extra", b: y.payload });
      continue;
    }
    if (!x || !y) continue;
    if (x.kind !== y.kind) {
      entries.push({ kind: `${x.kind}≠${y.kind}`, diff: "changed", a: x.payload, b: y.payload });
      continue;
    }
    entries.push({
      kind: x.kind,
      diff: x.payload_hash === y.payload_hash ? "identical" : "changed",
      a: x.payload,
      b: y.payload,
    });
  }
  return { a: a[0]?.run_id ?? "", b: b[0]?.run_id ?? "", entries };
}
