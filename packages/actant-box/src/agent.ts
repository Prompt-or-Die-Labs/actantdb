/**
 * @actantdb/box — agent namespace.
 *
 * `box.agent.run` lazily wraps the user-supplied agent with `withActant` so
 * every model call + tool call lands in the box's ledger. Cost stays at
 * zero in local mode (we don't pretend to infer token counts).
 *
 * Streaming: if the underlying agent exposes a `stream` function, we
 * pass-through; otherwise we synthesize a single `finish` chunk from
 * `agent.generate`.
 */

import { withActant, type MastraAgentLike, type WrappedAgent } from "@actantdb/mastra";
import type { Ledger } from "@actantdb/core";
import type { Policy } from "@actantdb/types";

import { BoxError } from "./errors.js";
import { Run } from "./run.js";
import type { AgentChunk, AgentConfig } from "./types.js";
import { getHarness } from "./harnesses/index.js";
import type { Harness } from "./harnesses/types.js";

export interface AgentCtx {
  ledger: Ledger;
  workspaceDir: string;
  cwd: string;
  project: string;
  storeRoot: string;
  /** Per-box ledger directory — `<storeRoot>/<boxId>/.actantdb`. */
  ledgerStoreDir: string;
  mode: "local" | "cloud";
}

type CtxProvider = () => AgentCtx;

export interface AgentRunInput {
  prompt?: string;
  /** Alternative to `prompt`: raw input forwarded to `agent.generate`. */
  input?: unknown;
  /** Optional JSON schema for the response. Forwarded to `agent.generate` if it accepts it. */
  responseSchema?: unknown;
  /** Soft timeout for the underlying generate; we wrap with Promise.race. */
  timeout?: number;
  /** Policy applied by Guard inside withActant. */
  policy?: Policy;
  /** Auto-approve all approval gates (handy for tests). */
  autoApprove?: boolean;
}

export class BoxAgentAPI {
  private wrapped: WrappedAgent<MastraAgentLike> | undefined;
  /** Stable handle the consumer can reassign tools onto. */
  agent: MastraAgentLike;
  /** Harness adapter (set when config.agent.harness is configured). */
  private harness: Harness | undefined;
  /** Per-harness model override (from config). */
  private harnessModel: string | undefined;
  /** Per-harness api-key override. */
  private harnessApiKey: string | undefined;
  /** Extra CLI args from config. */
  private harnessExtraArgs: string[] | undefined;

  constructor(
    private readonly ctx: CtxProvider,
    initialAgent?: MastraAgentLike,
  ) {
    this.agent = initialAgent ?? { tools: {} };
  }

  /** Configure a preset CLI harness (Agent.ClaudeCode / Codex / OpenCode). */
  setHarness(config: AgentConfig): void {
    if (!config.harness) {
      this.harness = undefined;
      return;
    }
    const h = getHarness(config.harness);
    if (!h) {
      throw new BoxError(
        "unknown_harness",
        `Unknown harness "${config.harness}". Choose one of: claude-code, codex, opencode, cursor, or omit for a custom agent.`,
      );
    }
    this.harness = h;
    this.harnessModel = config.model;
    this.harnessApiKey = config.apiKey;
    this.harnessExtraArgs = config.extraArgs;
  }

  /** Swap the underlying agent. Resets the cached wrapper. */
  setAgent(agent: MastraAgentLike): void {
    this.agent = agent;
    this.wrapped = undefined;
  }

  /** Close the wrapper's underlying ledger handle (called by box.delete). */
  close(): void {
    try {
      this.wrapped?.actant.close();
    } catch {
      /* the box.ts handle is closed separately */
    }
    this.wrapped = undefined;
  }

  async run(input: AgentRunInput): Promise<Run> {
    const ctx = this.ctx();
    if (ctx.mode === "cloud") {
      throw new BoxError(
        "cloud_unsupported",
        "box.agent.run: cloud control plane is in development — see docs/CLOUD_ROADMAP.md Phase 2",
      );
    }
    // Harness path: spawn the configured CLI in the workspace, record the
    // result as a tool_call_completed event so the run lands in the ledger
    // just like a custom-agent path would.
    if (this.harness) {
      if (!input.prompt) {
        throw new BoxError(
          "exec_failed",
          `box.agent.run requires a prompt when using harness "${this.harness.name}"`,
        );
      }
      const t0 = performance.now();
      const res = await this.harness.run({
        prompt: input.prompt,
        cwd: `${ctx.workspaceDir}${ctx.cwd ? "/" + ctx.cwd : ""}`,
        model: this.harnessModel ?? this.harness.defaultModel,
        ...(this.harnessApiKey !== undefined ? { apiKey: this.harnessApiKey } : {}),
        ...(input.timeout !== undefined ? { timeoutMs: input.timeout } : {}),
        ...(this.harnessExtraArgs !== undefined ? { extraArgs: this.harnessExtraArgs } : {}),
      });
      const runId = `agent-${Date.now()}`;
      const r = new Run({ id: runId, ledger: ctx.ledger });
      if (res.ok) r.complete(res.result);
      else r.fail(new Error(`harness "${this.harness.name}" exited ${res.exitCode}: ${res.output.slice(-2000)}`));
      r.cost = { ...r.cost, computeMs: res.computeMs };
      return r;
    }

    const wrapped = this.ensureWrapped(input.policy, input.autoApprove);
    const ledger = ctx.ledger;
    const t0 = performance.now();

    const generatePayload = buildGeneratePayload(input);

    const runPromise = wrapped.run(generatePayload);
    const finalPromise =
      input.timeout && input.timeout > 0
        ? Promise.race([
            runPromise,
            new Promise<never>((_res, rej) =>
              setTimeout(
                () =>
                  rej(
                    new BoxError(
                      "exec_failed",
                      `box.agent.run timed out after ${input.timeout}ms`,
                    ),
                  ),
                input.timeout,
              ),
            ),
          ])
        : runPromise;

    try {
      const { runId, result } = await finalPromise;
      const run = new Run({ id: runId, ledger }).markRunning().complete(result);
      run.cost = {
        ...run.cost,
        computeMs: Math.round(performance.now() - t0),
      };
      return run;
    } catch (err) {
      const failedRun = new Run({ id: `agent-${Date.now()}`, ledger }).fail(err);
      if (err instanceof BoxError) throw err;
      void failedRun;
      throw new BoxError(
        "exec_failed",
        `box.agent.run failed: ${(err as Error).message}`,
        err,
      );
    }
  }

  async *stream(input: AgentRunInput): AsyncIterable<AgentChunk> {
    const ctx = this.ctx();
    ensureLocalAgentCtx(ctx, "box.agent.stream");
    if (this.harness) {
      yield* this.streamHarness(input, ctx);
      return;
    }

    const wrapped = this.ensureWrapped(input.policy, input.autoApprove);
    const generatePayload = buildGeneratePayload(input);
    const maybeStream = nativeStream(wrapped.agent);

    // Native streaming path: if the user-supplied agent has `stream`, forward.
    if (maybeStream) {
      yield* streamNativeAgent(wrapped, maybeStream, input, generatePayload);
      return;
    }

    // Fallback: run, yield a single finish chunk.
    yield* streamGeneratedResult(wrapped, generatePayload);
  }

  private async *streamHarness(
    input: AgentRunInput,
    ctx: AgentCtx,
  ): AsyncIterable<AgentChunk> {
    if (!this.harness) return;
    if (!input.prompt) {
      throw new BoxError(
        "exec_failed",
        `box.agent.stream requires a prompt when using harness "${this.harness.name}"`,
      );
    }
    yield* this.harness.stream({
      prompt: input.prompt,
      cwd: agentCwd(ctx),
      model: this.harnessModel ?? this.harness.defaultModel,
      ...(this.harnessApiKey !== undefined ? { apiKey: this.harnessApiKey } : {}),
      ...(input.timeout !== undefined ? { timeoutMs: input.timeout } : {}),
      ...(this.harnessExtraArgs !== undefined ? { extraArgs: this.harnessExtraArgs } : {}),
    });
  }

  private ensureWrapped(policy?: Policy, autoApprove?: boolean): WrappedAgent<MastraAgentLike> {
    const ctx = this.ctx();
    if (ctx.mode === "cloud") {
      throw new BoxError(
        "cloud_unsupported",
        "box.agent.run: cloud control plane is in development — see docs/CLOUD_ROADMAP.md Phase 2",
      );
    }
    if (this.wrapped) return this.wrapped;
    if (!this.agent || !this.agent.tools) {
      // We still wrap an empty-tools agent — useful when the consumer wants
      // `box.agent.run` to record a model_call without any tool calls.
    }
    this.wrapped = withActant(this.agent, {
      project: ctx.project,
      storeDir: ctx.ledgerStoreDir,
      ...(policy !== undefined ? { policy } : {}),
      ...(autoApprove !== undefined ? { autoApprove } : {}),
    });
    return this.wrapped;
  }
}

// ----- helpers -----

type NativeStream = (input: unknown) => AsyncIterable<unknown>;

function ensureLocalAgentCtx(ctx: AgentCtx, operation: string): void {
  if (ctx.mode !== "cloud") return;
  throw new BoxError(
    "cloud_unsupported",
    `${operation}: cloud control plane is in development — see docs/CLOUD_ROADMAP.md Phase 2`,
  );
}

function agentCwd(ctx: AgentCtx): string {
  return `${ctx.workspaceDir}${ctx.cwd ? "/" + ctx.cwd : ""}`;
}

function buildGeneratePayload(input: AgentRunInput): { message?: string; input?: unknown } {
  const payload: { message?: string; input?: unknown } = {};
  if (input.prompt !== undefined) payload.message = input.prompt;
  if (input.input !== undefined) payload.input = input.input;
  if (input.responseSchema !== undefined) {
    payload.input = { ...(payload.input as object | undefined), responseSchema: input.responseSchema };
  }
  return payload;
}

function normalizeChunk(raw: unknown): AgentChunk {
  if (typeof raw === "string") return { type: "text-delta", text: raw };
  if (raw && typeof raw === "object" && "type" in raw) {
    const r = raw as AgentChunk;
    return r;
  }
  return { type: "text-delta", text: String(raw) };
}

function nativeStream(agent: MastraAgentLike): NativeStream | undefined {
  const stream = (agent as { stream?: NativeStream }).stream;
  return typeof stream === "function" ? stream.bind(agent) : undefined;
}

async function* streamNativeAgent(
  wrapped: WrappedAgent<MastraAgentLike>,
  stream: NativeStream,
  input: AgentRunInput,
  generatePayload: { message?: string; input?: unknown },
): AsyncIterable<AgentChunk> {
  const ctx = wrapped.startRun();
  if (input.prompt !== undefined) ctx.recordUserMessage(input.prompt);
  try {
    for await (const raw of stream(generatePayload.input ?? generatePayload.message)) {
      yield normalizeChunk(raw);
    }
    ctx.finish({ ok: true });
  } catch (err) {
    ctx.finish({ ok: false, error: (err as Error).message ?? String(err) });
    throw err;
  }
}

async function* streamGeneratedResult(
  wrapped: WrappedAgent<MastraAgentLike>,
  generatePayload: { message?: string; input?: unknown },
): AsyncIterable<AgentChunk> {
  const { result } = await wrapped.run(generatePayload);
  if (typeof result === "string") yield { type: "text-delta", text: result };
  yield { type: "finish", result };
}
