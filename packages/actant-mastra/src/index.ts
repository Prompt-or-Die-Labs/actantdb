/**
 * @actantdb/mastra — wrap a Mastra agent (or any agent that follows the same
 * tool shape) for ActantDB capture, approval, and replay.
 *
 * Phase 1 surface (per /wedge/60-day-plan.md):
 *
 *   const wrapped = withActant(agent, {
 *     project: "support-agent",
 *     policy: demoPolicy,
 *   });
 *
 *   await wrapped.run({ message: "Clean up the test artifacts." });
 *
 * We deliberately don't take a hard dependency on `@mastra/core`'s class
 * surface so the wrapper works with any agent exposing a tools record. A
 * Mastra `Agent` matches this shape today.
 */

import {
  createActant,
  buildContextManifest,
  ulid,
  type ActantHandle,
  type RunContext,
} from "@actantdb/core";
import { evaluate, snapshotHash } from "@actantdb/policy";
import type {
  ApprovalDecision,
  ContextManifest,
  ModelCall,
  Policy,
  PolicyVerdict,
  ToolCallRequest,
} from "@actantdb/types";

/** Minimum tool shape we wrap. */
export interface MastraToolLike {
  /** Tool name (e.g. "shell.run"). Optional; falls back to record key. */
  id?: string;
  /** Tool description. */
  description?: string;
  /** Execute is replaced by our wrapper. */
  execute: (args: unknown, ctx?: unknown) => Promise<unknown>;
}

/** Minimum agent shape we wrap. */
export interface MastraAgentLike {
  /** Agent name. */
  name?: string;
  /** Record of tools the agent can call. */
  tools?: Record<string, MastraToolLike>;
  /** Optional generate function; if present we capture model_call events. */
  generate?: (input: unknown, opts?: unknown) => Promise<unknown>;
}

/** Options for `withActant`. */
export interface WithActantOptions {
  /** Project identifier used for the local store. */
  project: string;
  /** Policy applied by Guard on every tool call. */
  policy?: Policy;
  /** Override storage root (default: ~/.actantdb). */
  storeDir?: string;
  /** Approval gate. The default policy implementation requires approval be
   *  explicitly attached for `require_approval` verdicts. Providing
   *  `autoApprove: true` accepts every approval (test harness convenience). */
  autoApprove?: boolean;
  /** Custom approval resolver (called when verdict is `require_approval`).
   *  If omitted, the call is left pending in the approvals queue and the
   *  tool call is short-circuited (status=Blocked from the run's view). */
  resolveApproval?: (req: ToolCallRequest, verdict: PolicyVerdict) => Promise<ApprovalDecision>;
}

/** Augmented agent returned by `withActant`. */
export interface WrappedAgent<A extends MastraAgentLike> {
  /** Original agent reference, kept for downstream usage. */
  readonly agent: A;
  /** Project handle that exposes the ledger + approvals API. */
  readonly actant: ActantHandle;
  /** Active policy snapshot hash (matches what verdicts will carry). */
  readonly policySnapshot: string;
  /**
   * Run the agent with capture turned on. Returns the run id and final
   * result (whatever `agent.generate` returned). For Phase 1, `input` may
   * also be a `{ messages: ... }` object passed straight through.
   */
  run(input: { message?: string; input?: unknown }, opts?: { runId?: string }): Promise<{
    runId: string;
    result: unknown;
  }>;
  /** Lower-level: start a new run and return the capture context. */
  startRun(opts?: { runId?: string; meta?: unknown }): RunContext;
}

/**
 * Wrap a Mastra-shaped agent so every tool call is captured, gated through
 * Guard, optionally approved, and recorded. The returned wrapper exposes
 * `.actant` and `.run()`; the underlying agent's tools are wrapped in-place.
 */
export function withActant<A extends MastraAgentLike>(
  agent: A,
  opts: WithActantOptions,
): WrappedAgent<A> {
  const actant = createActant({
    project: opts.project,
    ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
    ...(opts.policy !== undefined ? { policy: opts.policy } : {}),
  });
  const policy = opts.policy ?? { tools: [], deny: [] };
  const policySnapshot = snapshotHash(policy);

  // Hold the active run context here while a run is in flight so wrapped
  // tools can see it (the agent likely calls `tool.execute()` synchronously
  // from inside `generate()`).
  let active: RunContext | undefined;

  // Replace each tool.execute with a gated wrapper. The wrapper invokes the
  // original execute with the (possibly constrain-rewritten) final args so
  // the underlying tool receives exactly what Guard sealed.
  if (agent.tools) {
    for (const [name, tool] of Object.entries(agent.tools)) {
      const original = tool.execute.bind(tool);
      const toolName = tool.id ?? name;
      tool.execute = async (args: unknown, ctx?: unknown) => {
        const run = active;
        if (!run) {
          // No active capture; fall through transparently.
          return original(args, ctx);
        }
        return runGatedTool({
          run,
          policy,
          toolName,
          args,
          execute: (finalArgs) => original(finalArgs, ctx),
          opts,
        });
      };
    }
  }

  function startRun(o?: { runId?: string; meta?: unknown }): RunContext {
    const ctx = actant.startRun(o);
    active = ctx;
    return ctx;
  }

  async function run(
    input: { message?: string; input?: unknown },
    o?: { runId?: string },
  ): Promise<{ runId: string; result: unknown }> {
    const ctx = startRun({
      ...(o?.runId !== undefined ? { runId: o.runId } : {}),
      meta: { input },
    });
    try {
      if (input.message !== undefined) ctx.recordUserMessage(input.message);
      let result: unknown = undefined;
      if (typeof agent.generate === "function") {
        // Capture a synthetic model_call so the timeline always has the
        // planner row the demo references — even if the agent doesn't.
        const summary =
          typeof input.message === "string" && input.message.length > 0
            ? input.message
            : "agent.generate()";
        const planner: ModelCall = {
          model: "user-provided",
          role: "planner",
          prompt_hash: "",
          summary,
        };
        ctx.recordModelCall(planner);
        result = await agent.generate(input.input ?? input.message ?? input, opts);
      }
      ctx.finish({ ok: true });
      return { runId: ctx.runId, result };
    } catch (err) {
      ctx.finish({ ok: false, error: (err as Error).message ?? String(err) });
      throw err;
    } finally {
      active = undefined;
    }
  }

  return {
    agent,
    actant,
    policySnapshot,
    run,
    startRun,
  };
}

/**
 * Internal: run a single tool call with Guard + optional approval.
 * Exported for testability.
 */
export async function runGatedTool(args: {
  run: RunContext;
  policy: Policy;
  toolName: string;
  args: unknown;
  /** Invoked with the final args Guard sealed (constrain-rewritten if any). */
  execute: (finalArgs: unknown) => Promise<unknown>;
  opts: WithActantOptions;
}): Promise<unknown> {
  const { run, policy, toolName, args: toolArgs, execute, opts } = args;
  const toolCallId = ulid();
  const req: ToolCallRequest = {
    tool_call_id: toolCallId,
    tool: toolName,
    args: toolArgs ?? {},
    risk: "low",
  };
  run.recordToolCallRequested(req);
  const v = evaluate(policy, req);
  run.recordGuardVerdict(toolCallId, v);

  const t0 = performance.now();

  if (v.decision === "halt") {
    run.recordToolCallCompleted({
      tool_call_id: toolCallId,
      status: "blocked",
      result: { reason: v.reason },
      duration_ms: Math.round(performance.now() - t0),
    });
    throw new Error(`Actant Guard halted run: ${v.reason}`);
  }

  if (v.decision === "block") {
    run.recordToolCallCompleted({
      tool_call_id: toolCallId,
      status: "blocked",
      result: { reason: v.reason },
      duration_ms: Math.round(performance.now() - t0),
    });
    return { blocked: true, reason: v.reason };
  }

  let finalArgs: unknown = toolArgs ?? {};
  if (v.decision === "constrain") {
    finalArgs = v.constrained_input;
  }

  if (v.decision === "require_approval") {
    const approvalReq = {
      tool_call_id: toolCallId,
      tool: req.tool,
      args: req.args,
      reason: v.reason,
      ...(v.hint !== undefined ? { hint: v.hint } : {}),
      ...(v.constrained_input !== undefined
        ? { constrained_input: v.constrained_input }
        : {}),
    };
    run.recordApprovalRequired(approvalReq);
    const decision = await resolveApproval(req, v, opts);
    run.recordApprovalDecision(toolCallId, decision);
    if (decision.decision === "deny") {
      run.recordToolCallCompleted({
        tool_call_id: toolCallId,
        status: "denied",
        result: { reason: decision.reason },
        duration_ms: Math.round(performance.now() - t0),
      });
      return { denied: true, reason: decision.reason };
    }
    if (decision.decision === "approve_constrained") {
      finalArgs = decision.accepted_input;
    }
  }

  run.recordToolCallStarted(toolCallId, finalArgs);
  try {
    const out = await execute(finalArgs);
    run.recordToolCallCompleted({
      tool_call_id: toolCallId,
      status: "ok",
      result: out as unknown as object,
      duration_ms: Math.round(performance.now() - t0),
    });
    return out;
  } catch (e) {
    run.recordToolCallCompleted({
      tool_call_id: toolCallId,
      status: "error",
      result: { error: (e as Error).message ?? String(e) },
      duration_ms: Math.round(performance.now() - t0),
    });
    throw e;
  }
}

async function resolveApproval(
  req: ToolCallRequest,
  v: PolicyVerdict,
  opts: WithActantOptions,
): Promise<ApprovalDecision> {
  if (opts.resolveApproval) return opts.resolveApproval(req, v);
  if (opts.autoApprove) {
    if (v.decision === "require_approval" && v.constrained_input !== undefined) {
      return {
        decision: "approve_constrained",
        approver: "auto",
        scope: "once",
        accepted_input: v.constrained_input,
      };
    }
    return { decision: "approve", approver: "auto", scope: "once" };
  }
  return {
    decision: "deny",
    approver: "system",
    reason: "no approver attached and autoApprove=false",
  };
}

/** Convenience: build a context manifest using the helper from @actantdb/core. */
export { buildContextManifest };
export type { ContextManifest };
