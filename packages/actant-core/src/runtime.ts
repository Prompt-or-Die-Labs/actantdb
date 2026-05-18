import { Ledger } from "./ledger.js";
import { ApprovalStore, type ApprovalRecord } from "./approvals.js";
import { ulid } from "./ulid.js";
import { sha256OfJSON } from "./hash.js";

import type {
  ActantEvent,
  ApprovalDecision,
  ApprovalRequest,
  ContextItem,
  ContextManifest,
  ModelCall,
  Policy,
  PolicyVerdict,
  ToolCallCompleted,
  ToolCallRequest,
} from "@actantdb/types";

/** Options for `createActant`. */
export interface ActantOptions {
  /** Project identifier. Required. */
  project: string;
  /** Local-only embedded mode (Phase 1 default). */
  mode?: "embedded";
  /** Override storage root (default: ~/.actantdb). */
  storeDir?: string;
  /** Optional policy applied by Guard. */
  policy?: Policy;
}

/** Public handle returned by `createActant`. */
export interface ActantHandle {
  readonly project: string;
  readonly ledger: Ledger;
  readonly approvals: ApprovalStore;
  /** Start a logical agent run. Returns a per-run capture context. */
  startRun(opts?: { runId?: string; meta?: unknown }): RunContext;
  /** Get the active policy snapshot hash (helpful for tests & replay). */
  policyHash(): string;
  /** Close underlying handles. */
  close(): void;
}

/** Per-run capture API. */
export interface RunContext {
  readonly runId: string;
  readonly project: string;
  /** Record a user-message-received event. */
  recordUserMessage(text: string): ActantEvent;
  /** Record a model-call event (planner / generator). */
  recordModelCall(info: ModelCall): ActantEvent;
  /** Record the context manifest fed to a model call. */
  recordContextBuild(manifest: ContextManifest): ActantEvent;
  /** Record a tool-call request. */
  recordToolCallRequested(req: ToolCallRequest): ActantEvent;
  /** Record a guard verdict for a tool call. */
  recordGuardVerdict(toolCallId: string, verdict: PolicyVerdict): ActantEvent;
  /** Record that an approval is required. */
  recordApprovalRequired(req: ApprovalRequest): { event: ActantEvent; record: ApprovalRecord };
  /** Record an approval decision. */
  recordApprovalDecision(
    toolCallId: string,
    decision: ApprovalDecision,
  ): { event: ActantEvent; record: ApprovalRecord };
  /** Record that a tool call started (post-Guard / post-approval). */
  recordToolCallStarted(toolCallId: string, finalArgs: unknown): ActantEvent;
  /** Record completion of a tool call. */
  recordToolCallCompleted(payload: ToolCallCompleted): ActantEvent;
  /** Record an arbitrary observation (effect_observed). */
  recordEffect(payload: unknown): ActantEvent;
  /** Record run completion. */
  finish(payload?: unknown): ActantEvent;
}

/** Construct a project-scoped Actant runtime. */
export function createActant(opts: ActantOptions): ActantHandle {
  const ledger = new Ledger({
    project: opts.project,
    ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
  });
  const approvals = new ApprovalStore(ledger);
  const policy = opts.policy ?? ({ tools: [], deny: [] } as Policy);
  const policySnapshot = sha256OfJSON(policy);
  return {
    project: opts.project,
    ledger,
    approvals,
    policyHash: () => policySnapshot,
    startRun(o) {
      const runId = o?.runId ?? ulid();
      // Genesis event
      ledger.append({
        kind: "agent_run_started",
        runId,
        payload: { project: opts.project, meta: o?.meta ?? null },
        sensitivity: "low",
      });
      return makeRunContext(runId, ledger, approvals);
    },
    close: () => ledger.close(),
  };
}

function makeRunContext(runId: string, ledger: Ledger, approvals: ApprovalStore): RunContext {
  const project = ledger.project;
  return {
    runId,
    project,
    recordUserMessage: (text) =>
      ledger.append({
        kind: "user_message_received",
        runId,
        payload: { text },
        sensitivity: "low",
      }),
    recordModelCall: (info) =>
      ledger.append({ kind: "model_call", runId, payload: info, sensitivity: "low" }),
    recordContextBuild: (manifest) =>
      ledger.append({ kind: "context_build", runId, payload: manifest, sensitivity: "medium" }),
    recordToolCallRequested: (req) =>
      ledger.append({
        kind: "tool_call_requested",
        runId,
        payload: req,
        sensitivity: "low",
      }),
    recordGuardVerdict: (toolCallId, verdict) =>
      ledger.append({
        kind: "guard_verdict",
        runId,
        payload: { tool_call_id: toolCallId, ...verdict },
        sensitivity: "low",
      }),
    recordApprovalRequired: (req) => {
      approvals.enqueue(req, runId);
      const event = ledger.append({
        kind: "approval_required",
        runId,
        payload: req,
        sensitivity: "low",
      });
      const record = approvals.get(req.tool_call_id);
      if (!record) throw new Error(`approval missing after enqueue: ${req.tool_call_id}`);
      return { event, record };
    },
    recordApprovalDecision: (toolCallId, decision) => {
      const record = approvals.decide(toolCallId, decision);
      const event = ledger.append({
        kind: "approval_decision",
        runId,
        payload: { tool_call_id: toolCallId, ...decision },
        sensitivity: "low",
      });
      return { event, record };
    },
    recordToolCallStarted: (toolCallId, finalArgs) =>
      ledger.append({
        kind: "tool_call_started",
        runId,
        payload: { tool_call_id: toolCallId, final_args: finalArgs },
        sensitivity: "low",
      }),
    recordToolCallCompleted: (payload) =>
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload,
        sensitivity: "low",
      }),
    recordEffect: (payload) =>
      ledger.append({ kind: "effect_observed", runId, payload, sensitivity: "low" }),
    finish: (payload) =>
      ledger.append({
        kind: "agent_run_finished",
        runId,
        payload: payload ?? {},
        sensitivity: "low",
      }),
  };
}

/** Compute a context manifest from raw items. Items are hashed by content. */
export function buildContextManifest(
  included: Array<Omit<ContextItem, "content_hash"> & { content: string }>,
  blocked: Array<Omit<ContextItem, "content_hash"> & { content: string }> = [],
): ContextManifest {
  const inc: ContextItem[] = included.map((i) => ({
    id: i.id,
    kind: i.kind,
    source: i.source,
    content_hash: sha256OfJSON(i.content),
    sensitivity: i.sensitivity,
    label: i.label,
    flags: i.flags ?? [],
  }));
  const blk: ContextItem[] = blocked.map((i) => ({
    id: i.id,
    kind: i.kind,
    source: i.source,
    content_hash: sha256OfJSON(i.content),
    sensitivity: i.sensitivity,
    label: i.label,
    flags: i.flags ?? [],
  }));
  const manifest_hash = sha256OfJSON({
    included: inc.map((i) => ({ id: i.id, content_hash: i.content_hash })),
  });
  return { manifest_hash, included: inc, ...(blk.length ? { blocked: blk } : {}) };
}
