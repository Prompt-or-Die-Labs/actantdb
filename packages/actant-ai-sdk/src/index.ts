/**
 * @actantdb/ai-sdk — wrap the Vercel AI SDK (`ai` package) so every
 * `streamText` / `generateText` / `generateObject` call lands as typed
 * events in the ActantDB ledger.
 *
 *   import { wrapAiSdk } from "@actantdb/ai-sdk";
 *   import { generateText } from "ai";
 *   import { openai } from "@ai-sdk/openai";
 *
 *   const wrapped = wrapAiSdk({
 *     project: "my-app",
 *     storeDir: "./.actantdb",
 *     ai: { generateText },   // or supply nothing to lazy-resolve from `ai`
 *   });
 *
 *   const result = await wrapped.generateText({
 *     model: openai("gpt-4o"),
 *     messages: [...],
 *     tools: { ... },
 *   });
 *
 * Each model call records `model_call`; each tool call records
 * `tool_call_requested` / `guard_verdict` / `tool_call_started` /
 * `tool_call_completed` via the same `runGatedTool` helper used by
 * `@actantdb/mastra`.
 *
 * The upstream `ai` package is an OPTIONAL peer dep — resolved lazily so
 * this package builds & runs even when the peer isn't installed.
 */

import { createRequire } from "node:module";
import {
  createActant,
  sha256OfJSON,
  type ActantHandle,
  type RunContext,
} from "@actantdb/core";
import { runGatedTool } from "@actantdb/mastra";
import type { ModelCall, Policy } from "@actantdb/types";

const requireFromHere = createRequire(import.meta.url);

/** Subset of the upstream `ai` package surface we wrap. */
export interface AiSdkLike {
  streamText?: (...args: unknown[]) => unknown;
  generateText?: (...args: unknown[]) => unknown;
  generateObject?: (...args: unknown[]) => unknown;
}

/** Tool entry shape (matches AI SDK v4+). */
export interface AiSdkToolLike {
  description?: string;
  parameters?: unknown;
  inputSchema?: unknown;
  execute?: (args: unknown, ctx?: unknown) => Promise<unknown>;
}

/** Common params accepted by streamText/generateText. */
export interface AiSdkCallParams {
  model?: unknown;
  messages?: unknown;
  prompt?: unknown;
  tools?: Record<string, AiSdkToolLike>;
  [key: string]: unknown;
}

/** Options for `wrapAiSdk`. */
export interface WrapAiSdkOptions {
  project: string;
  storeDir?: string;
  /** Reuse an existing handle (skip auto-create). */
  handle?: ActantHandle;
  /** Reuse a RunContext from `@actantdb/mastra` / `@actantdb/core`. */
  run?: RunContext;
  /** Active policy applied by Guard for each tool call. */
  policy?: Policy;
  /** Approval shortcut for `require_approval` verdicts. */
  autoApprove?: boolean;
  /** Optional resolver for approvals. */
  resolveApproval?: Parameters<typeof runGatedTool>[0]["opts"]["resolveApproval"];
  /** Inject the upstream `ai` module (test escape hatch). */
  ai?: AiSdkLike;
}

/** Surface returned by `wrapAiSdk`. */
export interface WrappedAiSdk {
  readonly actant: ActantHandle;
  /** Wrapped `streamText`. */
  streamText(params: AiSdkCallParams): unknown;
  /** Wrapped `generateText`. */
  generateText(params: AiSdkCallParams): unknown;
  /** Wrapped `generateObject`. */
  generateObject(params: AiSdkCallParams): unknown;
  /** Lower-level: start a run and return its context. */
  startRun(opts?: { runId?: string; meta?: unknown }): RunContext;
  /** Close ledger if `wrapAiSdk` created it. */
  close(): void;
}

function resolveAi(): AiSdkLike {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = requireFromHere("ai") as AiSdkLike;
    return mod;
  } catch (err) {
    throw new Error(
      "@actantdb/ai-sdk requires the `ai` package to be installed alongside it. " +
        "Install it with `npm install ai`. " +
        `(underlying error: ${(err as Error).message})`,
    );
  }
}

function summariseMessages(params: AiSdkCallParams): string {
  if (typeof params.prompt === "string") return params.prompt.slice(0, 200);
  const msgs = params.messages;
  if (Array.isArray(msgs) && msgs.length > 0) {
    const last = msgs[msgs.length - 1] as { content?: unknown };
    if (last && typeof last === "object" && "content" in last) {
      const c = last.content;
      if (typeof c === "string") return c.slice(0, 200);
    }
  }
  return "ai-sdk.call()";
}

function modelLabel(model: unknown): string {
  if (typeof model === "string") return model;
  if (model && typeof model === "object") {
    const m = model as { modelId?: unknown; provider?: unknown };
    if (typeof m.modelId === "string") {
      return typeof m.provider === "string"
        ? `${m.provider}:${m.modelId}`
        : m.modelId;
    }
  }
  return "ai-sdk:unknown";
}

/**
 * Wrap the Vercel AI SDK for ActantDB capture. Returns a small facade that
 * exposes `streamText` / `generateText` / `generateObject`, each of which
 * forwards to the upstream after rewriting `tools[*].execute` so Guard +
 * approval are enforced and every call records the right events.
 */
export function wrapAiSdk(opts: WrapAiSdkOptions): WrappedAiSdk {
  const ai = opts.ai ?? resolveAi();
  const handle: ActantHandle =
    opts.handle ??
    createActant({
      project: opts.project,
      ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
      ...(opts.policy !== undefined ? { policy: opts.policy } : {}),
    });
  const ownsHandle = opts.handle === undefined;
  const policy: Policy = opts.policy ?? { tools: [], deny: [] };

  let active: RunContext | undefined = opts.run;

  function startRun(o?: { runId?: string; meta?: unknown }): RunContext {
    const ctx = handle.startRun(o);
    active = ctx;
    return ctx;
  }

  function ensureRun(meta: unknown): { run: RunContext; adHoc: boolean } {
    if (active) return { run: active, adHoc: false };
    const run = handle.startRun({ meta });
    active = run;
    return { run, adHoc: true };
  }

  function wrapTools(
    tools: Record<string, AiSdkToolLike> | undefined,
    run: RunContext,
  ): Record<string, AiSdkToolLike> | undefined {
    if (!tools) return undefined;
    const out: Record<string, AiSdkToolLike> = {};
    for (const [name, tool] of Object.entries(tools)) {
      const original = tool.execute;
      if (!original) {
        out[name] = tool;
        continue;
      }
      const bound = original.bind(tool);
      out[name] = {
        ...tool,
        execute: async (args: unknown, ctx?: unknown) =>
          runGatedTool({
            run,
            policy,
            toolName: name,
            args,
            execute: (finalArgs) => bound(finalArgs, ctx),
            opts: {
              project: opts.project,
              ...(opts.autoApprove !== undefined
                ? { autoApprove: opts.autoApprove }
                : {}),
              ...(opts.resolveApproval !== undefined
                ? { resolveApproval: opts.resolveApproval }
                : {}),
            },
          }),
      };
    }
    return out;
  }

  function runRecorded(
    surface: "streamText" | "generateText" | "generateObject",
    params: AiSdkCallParams,
  ): unknown {
    const fn = ai[surface];
    if (typeof fn !== "function") {
      throw new Error(
        `@actantdb/ai-sdk: upstream \`ai\` package does not expose \`${surface}\`.`,
      );
    }
    const { run, adHoc } = ensureRun({
      source: "@actantdb/ai-sdk",
      surface,
    });
    const promptHash = sha256OfJSON(params.messages ?? params.prompt ?? params);
    const summary = summariseMessages(params);
    const event: ModelCall = {
      model: modelLabel(params.model),
      role: "generator",
      prompt_hash: promptHash,
      summary,
    };
    run.recordModelCall(event);
    const wrappedTools = wrapTools(params.tools, run);
    const forwardedParams: AiSdkCallParams = {
      ...params,
      ...(wrappedTools !== undefined ? { tools: wrappedTools } : {}),
    };

    const result = fn(forwardedParams);
    // For streamText, upstream returns a stream-shaped object synchronously.
    // For generateText/generateObject, upstream returns a Promise. We
    // attach completion handlers in both cases without blocking.
    if (result && typeof (result as Promise<unknown>).then === "function") {
      (result as Promise<unknown>)
        .then(() => {
          if (adHoc) run.finish({ ok: true });
        })
        .catch((err: unknown) => {
          if (adHoc)
            run.finish({
              ok: false,
              error: (err as Error).message ?? String(err),
            });
        });
    } else if (adHoc) {
      // Stream: best-effort — close the run when the consumer finishes by
      // awaiting `result.text` / `result.finishReason` if present.
      const r = result as { text?: Promise<unknown>; finishReason?: Promise<unknown> };
      const tail = r?.text ?? r?.finishReason;
      if (tail && typeof (tail as Promise<unknown>).then === "function") {
        (tail as Promise<unknown>)
          .then(() => run.finish({ ok: true }))
          .catch((err: unknown) =>
            run.finish({
              ok: false,
              error: (err as Error).message ?? String(err),
            }),
          );
      } else {
        // Nothing to await — leave the run open for the caller to finish.
      }
    }
    return result;
  }

  return {
    actant: handle,
    startRun,
    streamText: (params) => runRecorded("streamText", params),
    generateText: (params) => runRecorded("generateText", params),
    generateObject: (params) => runRecorded("generateObject", params),
    close: () => {
      if (ownsHandle) handle.close();
    },
  };
}
