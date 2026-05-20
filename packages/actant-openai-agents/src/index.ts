/**
 * @actantdb/openai-agents — wrap an `@openai/agents` SDK `Agent` so every
 * model call and tool call lands as typed events in the ActantDB ledger.
 *
 *   import { Agent } from "@openai/agents";
 *   import { withActantAgent } from "@actantdb/openai-agents";
 *
 *   const myAgent = new Agent({
 *     model: "gpt-4o",
 *     tools: [...],
 *     instructions: "...",
 *   });
 *
 *   const wrapped = withActantAgent(myAgent, {
 *     project: "support-bot",
 *     storeDir: "./.actantdb",
 *     policy: myPolicy,
 *     autoApprove: true,
 *   });
 *
 *   const { result, runId } = await wrapped.run({ input: "hello" });
 *
 * The wrapper:
 *  - intercepts each `tool.invoke()` (or `tool.execute()` on older
 *    revisions) via a Proxy and routes it through `runGatedTool`,
 *  - synthesises a top-level `model_call` event so the run timeline is
 *    coherent even when the upstream SDK doesn't expose per-call hooks,
 *  - forwards everything else verbatim (`agent.asTool`, `agent.clone`,
 *    `agent.handoffs`, etc.).
 *
 * The `@openai/agents` SDK is a peer dep but is OPTIONAL — this package
 * builds & runs even when it isn't installed; you supply your own agent
 * instance at runtime.
 */

import {
  createActant,
  type ActantHandle,
  type RunContext,
} from "@actantdb/core";
import { runGatedTool } from "@actantdb/mastra";
import type { ModelCall, Policy } from "@actantdb/types";

/** Minimum tool shape we wrap. Matches both `tool({ name, execute })` and
 *  the v0.x `Tool` class. */
export interface OpenAIAgentToolLike {
  name?: string;
  invoke?: (args: unknown, ctx?: unknown) => Promise<unknown>;
  execute?: (args: unknown, ctx?: unknown) => Promise<unknown>;
}

/** Minimum agent shape we wrap. */
export interface OpenAIAgentLike {
  name?: string;
  model?: unknown;
  tools?: OpenAIAgentToolLike[];
  run?: (input: unknown, opts?: unknown) => Promise<unknown>;
  /** SDK v0.0.x sometimes exposes a class method `runSync`. */
  runSync?: (input: unknown, opts?: unknown) => unknown;
}

/** Options for `withActantAgent`. */
export interface WithActantAgentOptions {
  project: string;
  storeDir?: string;
  handle?: ActantHandle;
  policy?: Policy;
  autoApprove?: boolean;
  resolveApproval?: Parameters<typeof runGatedTool>[0]["opts"]["resolveApproval"];
}

/** Augmented agent returned by `withActantAgent`. */
export interface WrappedOpenAIAgent<A extends OpenAIAgentLike> {
  readonly agent: A;
  readonly actant: ActantHandle;
  /** Run the agent with capture turned on. */
  run(input: {
    input?: unknown;
    message?: string;
  }, opts?: { runId?: string }): Promise<{ runId: string; result: unknown }>;
  /** Lower-level: start a run and return its context. */
  startRun(opts?: { runId?: string; meta?: unknown }): RunContext;
  /** Close the underlying ledger if this wrapper created it. */
  close(): void;
}

function modelLabel(model: unknown): string {
  if (typeof model === "string") return model;
  if (model && typeof model === "object") {
    const m = model as { modelId?: unknown; name?: unknown };
    if (typeof m.modelId === "string") return m.modelId;
    if (typeof m.name === "string") return m.name;
  }
  return "openai-agents:unknown";
}

/**
 * Wrap an `@openai/agents` agent. Tools are mutated in place so the
 * agent's own `.run()` path picks them up. Returns the wrapper with
 * `.run()`, `.actant`, etc.
 */
export function withActantAgent<A extends OpenAIAgentLike>(
  agent: A,
  opts: WithActantAgentOptions,
): WrappedOpenAIAgent<A> {
  const handle = actantHandleFor(opts);
  const ownsHandle = opts.handle === undefined;
  const policy: Policy = opts.policy ?? { tools: [], deny: [] };
  const active: ActiveRun = { current: undefined };

  wrapOpenAITools(agent, policy, gateOptions(opts), active);

  function startRun(o?: { runId?: string; meta?: unknown }): RunContext {
    const ctx = handle.startRun(o);
    active.current = ctx;
    return ctx;
  }

  return {
    agent,
    actant: handle,
    run: (input, runOpts) => runOpenAIAgent(agent, startRun, active, input, runOpts),
    startRun,
    close: () => {
      if (ownsHandle) handle.close();
    },
  };
}

type GateOptions = Parameters<typeof runGatedTool>[0]["opts"];
type ToolRunner = (args: unknown, ctx?: unknown) => Promise<unknown>;

interface ActiveRun {
  current: RunContext | undefined;
}

function actantHandleFor(opts: WithActantAgentOptions): ActantHandle {
  return (
    opts.handle ??
    createActant({
      project: opts.project,
      ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
      ...(opts.policy !== undefined ? { policy: opts.policy } : {}),
    })
  );
}

function gateOptions(opts: WithActantAgentOptions): GateOptions {
  return {
    project: opts.project,
    ...(opts.autoApprove !== undefined ? { autoApprove: opts.autoApprove } : {}),
    ...(opts.resolveApproval !== undefined
      ? { resolveApproval: opts.resolveApproval }
      : {}),
  };
}

function wrapOpenAITools<A extends OpenAIAgentLike>(
  agent: A,
  policy: Policy,
  gateOpts: GateOptions,
  active: ActiveRun,
): void {
  if (!Array.isArray(agent.tools)) return;
  for (let i = 0; i < agent.tools.length; i++) {
    wrapOpenAITool(agent.tools[i], i, policy, gateOpts, active);
  }
}

function wrapOpenAITool(
  tool: OpenAIAgentToolLike | undefined,
  index: number,
  policy: Policy,
  gateOpts: GateOptions,
  active: ActiveRun,
): void {
  if (!tool) return;
  const original = originalToolRunner(tool);
  if (!original) return;
  const gated = gatedToolRunner(toolName(tool, index), original, policy, gateOpts, active);
  if (tool.invoke) tool.invoke = gated;
  if (tool.execute) tool.execute = gated;
}

function originalToolRunner(tool: OpenAIAgentToolLike): ToolRunner | undefined {
  const origInvoke = tool.invoke ? tool.invoke.bind(tool) : undefined;
  const origExecute = tool.execute ? tool.execute.bind(tool) : undefined;
  return origInvoke ?? origExecute;
}

function toolName(tool: OpenAIAgentToolLike, index: number): string {
  const id = (tool as { id?: unknown }).id;
  if (tool.name) return tool.name;
  if (typeof id === "string") return id;
  return `tool_${index}`;
}

function gatedToolRunner(
  toolName: string,
  original: ToolRunner,
  policy: Policy,
  gateOpts: GateOptions,
  active: ActiveRun,
): ToolRunner {
  return async (args, ctx) => {
    const run = active.current;
    if (!run) return original(args, ctx);
    return runGatedTool({
      run,
      policy,
      toolName,
      args,
      execute: (finalArgs) => original(finalArgs, ctx),
      opts: gateOpts,
    });
  };
}

async function runOpenAIAgent<A extends OpenAIAgentLike>(
  agent: A,
  startRun: (opts?: { runId?: string; meta?: unknown }) => RunContext,
  active: ActiveRun,
  input: { input?: unknown; message?: string },
  opts?: { runId?: string },
): Promise<{ runId: string; result: unknown }> {
  const ctx = startRun({
    ...(opts?.runId !== undefined ? { runId: opts.runId } : {}),
    meta: { input },
  });
  try {
    if (typeof input.message === "string") ctx.recordUserMessage(input.message);
    ctx.recordModelCall(plannerCall(agent, input));
    const result = await runAgent(agent, input);
    ctx.finish({ ok: true });
    return { runId: ctx.runId, result };
  } catch (err) {
    ctx.finish({ ok: false, error: (err as Error).message ?? String(err) });
    throw err;
  } finally {
    active.current = undefined;
  }
}

function plannerCall(agent: OpenAIAgentLike, input: { input?: unknown; message?: string }): ModelCall {
  return {
    model: modelLabel(agent.model),
    role: "planner",
    prompt_hash: "",
    summary: plannerSummary(input),
  };
}

function plannerSummary(input: { input?: unknown; message?: string }): string {
  if (typeof input.message === "string" && input.message.length > 0) return input.message;
  if (typeof input.input === "string") return input.input;
  return "openai-agents.run()";
}

async function runAgent(
  agent: OpenAIAgentLike,
  input: { input?: unknown; message?: string },
): Promise<unknown> {
  const payload = input.input ?? input.message ?? input;
  if (typeof agent.run === "function") return agent.run(payload);
  if (typeof agent.runSync === "function") return agent.runSync(payload);
  return undefined;
}
