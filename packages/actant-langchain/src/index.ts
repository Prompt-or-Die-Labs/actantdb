/**
 * @actantdb/langchain — a LangChain JS `BaseCallbackHandler`-compatible
 * handler that records every LLM, chain, and tool event to the
 * ActantDB ledger.
 *
 *   import { ActantCallbackHandler } from "@actantdb/langchain";
 *   import { ChatAnthropic } from "@langchain/anthropic";
 *
 *   const handler = new ActantCallbackHandler({
 *     project: "my-app",
 *     storeDir: "./.actantdb",
 *   });
 *
 *   const chat = new ChatAnthropic({
 *     callbacks: [handler],
 *   });
 *
 * We implement the callback methods structurally (`handleLLMStart`,
 * `handleLLMEnd`, `handleToolStart`, `handleToolEnd`, etc.) so the
 * handler is duck-typed compatible with `@langchain/core/callbacks/base`
 * without requiring it to be installed at build time.
 */

import {
  createActant,
  sha256OfJSON,
  ulid,
  type ActantHandle,
  type RunContext,
} from "@actantdb/core";
import type { ModelCall, ToolCallRequest } from "@actantdb/types";

/** Options for `ActantCallbackHandler`. */
export interface ActantCallbackHandlerOptions {
  project: string;
  storeDir?: string;
  handle?: ActantHandle;
  /** Reuse an existing run; otherwise an ad-hoc run is opened on first
   *  event and closed when the LangChain chain finishes. */
  run?: RunContext;
}

interface ToolStartFrame {
  toolCallId: string;
  startedAtMs: number;
  name: string;
}

interface LlmStartFrame {
  startedAtMs: number;
  model: string;
  promptHash: string;
  summary: string;
}

/**
 * LangChain-compatible callback handler. Structurally satisfies the
 * `BaseCallbackHandler` contract without depending on `@langchain/core`
 * at build time.
 */
export class ActantCallbackHandler {
  readonly name = "actantdb_callback_handler";
  readonly actant: ActantHandle;
  readonly #ownsHandle: boolean;
  #activeRun: RunContext | undefined;
  #adHocRun: boolean = false;
  readonly #toolFrames = new Map<string, ToolStartFrame>();
  readonly #llmFrames = new Map<string, LlmStartFrame>();

  constructor(opts: ActantCallbackHandlerOptions) {
    if (opts.handle) {
      this.actant = opts.handle;
      this.#ownsHandle = false;
    } else {
      this.actant = createActant({
        project: opts.project,
        ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
      });
      this.#ownsHandle = true;
    }
    if (opts.run) {
      this.#activeRun = opts.run;
      this.#adHocRun = false;
    }
  }

  /** Close the ledger if this handler created it. */
  close(): void {
    if (this.#ownsHandle) this.actant.close();
  }

  #ensureRun(meta: unknown): RunContext {
    if (this.#activeRun) return this.#activeRun;
    const run = this.actant.startRun({ meta });
    this.#activeRun = run;
    this.#adHocRun = true;
    return run;
  }

  // ── LLM lifecycle ────────────────────────────────────────────────────

  handleLLMStart(
    llm: { id?: string[]; name?: string } | undefined,
    prompts: string[],
    runId: string,
    _parentRunId?: string,
    extraParams?: Record<string, unknown>,
    _tags?: string[],
    metadata?: Record<string, unknown>,
    name?: string,
  ): void {
    this.#ensureRun({ source: "@actantdb/langchain", surface: "llm" });
    const model =
      (extraParams &&
        (extraParams["model"] as string | undefined ?? (extraParams["modelName"] as string | undefined))) ??
      (metadata && (metadata["model"] as string | undefined)) ??
      name ??
      (Array.isArray(llm?.id) ? llm!.id![llm!.id!.length - 1] : llm?.name) ??
      "langchain:unknown";
    const promptHash = sha256OfJSON(prompts);
    const first = prompts[0] ?? "";
    this.#llmFrames.set(runId, {
      startedAtMs: performance.now(),
      model: String(model),
      promptHash,
      summary: first.slice(0, 200) || "llm.invoke()",
    });
  }

  handleChatModelStart(
    llm: { id?: string[]; name?: string } | undefined,
    messages: Array<Array<{ content?: unknown }>>,
    runId: string,
    _parentRunId?: string,
    extraParams?: Record<string, unknown>,
    _tags?: string[],
    metadata?: Record<string, unknown>,
    name?: string,
  ): void {
    this.#ensureRun({ source: "@actantdb/langchain", surface: "chat" });
    const model =
      (extraParams && (extraParams["model"] as string | undefined ?? (extraParams["modelName"] as string | undefined))) ??
      (metadata && (metadata["model"] as string | undefined)) ??
      name ??
      (Array.isArray(llm?.id) ? llm!.id![llm!.id!.length - 1] : llm?.name) ??
      "langchain:chat:unknown";
    const promptHash = sha256OfJSON(messages);
    const flat = (messages[0] ?? [])
      .map((m) => (typeof m.content === "string" ? m.content : ""))
      .filter(Boolean)
      .join(" / ");
    this.#llmFrames.set(runId, {
      startedAtMs: performance.now(),
      model: String(model),
      promptHash,
      summary: flat.slice(0, 200) || "chat.invoke()",
    });
  }

  handleLLMEnd(output: unknown, runId: string): void {
    const frame = this.#llmFrames.get(runId);
    if (!frame || !this.#activeRun) return;
    this.#llmFrames.delete(runId);
    const usage = extractLangchainUsage(output);
    const event: ModelCall = {
      model: frame.model,
      role: "generator",
      prompt_hash: frame.promptHash,
      summary: frame.summary,
      ...(usage.tokens_in !== undefined ? { tokens_in: usage.tokens_in } : {}),
      ...(usage.tokens_out !== undefined ? { tokens_out: usage.tokens_out } : {}),
    };
    this.#activeRun.recordModelCall(event);
  }

  handleLLMError(err: Error, runId: string): void {
    const frame = this.#llmFrames.get(runId);
    if (!frame || !this.#activeRun) return;
    this.#llmFrames.delete(runId);
    const event: ModelCall = {
      model: frame.model,
      role: "generator",
      prompt_hash: frame.promptHash,
      summary: `ERROR: ${err.message ?? String(err)}`,
    };
    this.#activeRun.recordModelCall(event);
  }

  // ── Tool lifecycle ───────────────────────────────────────────────────

  handleToolStart(
    tool: { id?: string[]; name?: string } | undefined,
    input: string,
    runId: string,
    _parentRunId?: string,
    _tags?: string[],
    _metadata?: Record<string, unknown>,
    name?: string,
  ): void {
    const run = this.#ensureRun({ source: "@actantdb/langchain", surface: "tool" });
    const toolName =
      name ??
      (Array.isArray(tool?.id) ? tool!.id![tool!.id!.length - 1] : tool?.name) ??
      "langchain:tool:unknown";
    const toolCallId = ulid();
    this.#toolFrames.set(runId, {
      toolCallId,
      startedAtMs: performance.now(),
      name: String(toolName),
    });
    let parsedInput: unknown = input;
    try {
      parsedInput = JSON.parse(input);
    } catch {
      // keep as string
    }
    const req: ToolCallRequest = {
      tool_call_id: toolCallId,
      tool: String(toolName),
      args: parsedInput ?? {},
      risk: "low",
    };
    run.recordToolCallRequested(req);
    run.recordToolCallStarted(toolCallId, parsedInput ?? {});
  }

  handleToolEnd(output: unknown, runId: string): void {
    const frame = this.#toolFrames.get(runId);
    if (!frame || !this.#activeRun) return;
    this.#toolFrames.delete(runId);
    this.#activeRun.recordToolCallCompleted({
      tool_call_id: frame.toolCallId,
      status: "ok",
      result: { output },
      duration_ms: Math.round(performance.now() - frame.startedAtMs),
    });
  }

  handleToolError(err: Error, runId: string): void {
    const frame = this.#toolFrames.get(runId);
    if (!frame || !this.#activeRun) return;
    this.#toolFrames.delete(runId);
    this.#activeRun.recordToolCallCompleted({
      tool_call_id: frame.toolCallId,
      status: "error",
      result: { error: err.message ?? String(err) },
      duration_ms: Math.round(performance.now() - frame.startedAtMs),
    });
  }

  // ── Chain lifecycle ──────────────────────────────────────────────────

  handleChainStart(
    chain: { id?: string[]; name?: string } | undefined,
    _inputs: unknown,
    _runId: string,
    parentRunId?: string,
  ): void {
    // Only open an ad-hoc run on the top-level chain so nested chains
    // don't blow up the timeline.
    if (parentRunId) return;
    this.#ensureRun({
      source: "@actantdb/langchain",
      surface: "chain",
      chain: chain?.name ?? (Array.isArray(chain?.id) ? chain!.id![chain!.id!.length - 1] : "unknown"),
    });
  }

  handleChainEnd(_output: unknown, _runId: string, parentRunId?: string): void {
    if (parentRunId) return;
    if (this.#adHocRun && this.#activeRun) {
      this.#activeRun.finish({ ok: true });
      this.#activeRun = undefined;
      this.#adHocRun = false;
    }
  }

  handleChainError(err: Error, _runId: string, parentRunId?: string): void {
    if (parentRunId) return;
    if (this.#adHocRun && this.#activeRun) {
      this.#activeRun.finish({
        ok: false,
        error: err.message ?? String(err),
      });
      this.#activeRun = undefined;
      this.#adHocRun = false;
    }
  }
}

function extractLangchainUsage(output: unknown): {
  tokens_in?: number;
  tokens_out?: number;
} {
  if (!output || typeof output !== "object") return {};
  const out: { tokens_in?: number; tokens_out?: number } = {};
  const o = output as {
    llmOutput?: { tokenUsage?: { promptTokens?: number; completionTokens?: number } };
    generations?: Array<Array<{ generationInfo?: { usage?: unknown }; message?: { usage_metadata?: { input_tokens?: number; output_tokens?: number } } }>>;
  };
  const tu = o.llmOutput?.tokenUsage;
  if (tu) {
    if (typeof tu.promptTokens === "number") out.tokens_in = tu.promptTokens;
    if (typeof tu.completionTokens === "number")
      out.tokens_out = tu.completionTokens;
    return out;
  }
  const usage = o.generations?.[0]?.[0]?.message?.usage_metadata;
  if (usage) {
    if (typeof usage.input_tokens === "number") out.tokens_in = usage.input_tokens;
    if (typeof usage.output_tokens === "number") out.tokens_out = usage.output_tokens;
  }
  return out;
}
