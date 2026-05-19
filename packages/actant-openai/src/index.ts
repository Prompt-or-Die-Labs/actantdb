/**
 * @actantdb/openai — drop-in replacement for the `openai` package that
 * records every `chat.completions.create()` and `responses.create()` call
 * as a `model_call` event in the ActantDB ledger.
 *
 *   import OpenAI from "@actantdb/openai";
 *
 *   const client = new OpenAI({
 *     apiKey: process.env.OPENAI_API_KEY,
 *     actant: { project: "my-app", storeDir: "./.actantdb" },
 *   });
 *
 *   const completion = await client.chat.completions.create({ ... });
 *
 * `completion` is the upstream response, byte-for-byte. A `model_call`
 * event is also written. Omit the `actant` field for a transparent
 * passthrough.
 *
 * The upstream `openai` package is an OPTIONAL peer dep — we resolve it
 * lazily via `createRequire` so this package builds & runs even when the
 * peer isn't installed.
 */

import { createRequire } from "node:module";
import {
  createActant,
  sha256OfJSON,
  type ActantHandle,
  type RunContext,
} from "@actantdb/core";
import type { ModelCall } from "@actantdb/types";

const requireFromHere = createRequire(import.meta.url);

/** ActantDB capture options attached to the constructor. */
export interface ActantClientOptions {
  project: string;
  storeDir?: string;
  handle?: ActantHandle;
  run?: RunContext;
  _upstream?: new (...args: unknown[]) => unknown;
}

export interface OpenAIConstructorOptions {
  [key: string]: unknown;
  actant?: ActantClientOptions;
}

interface UpstreamOpenAI {
  chat?: {
    completions?: {
      create: (...args: unknown[]) => Promise<unknown>;
    };
  };
  responses?: {
    create: (...args: unknown[]) => Promise<unknown>;
  };
  [key: string]: unknown;
}

function resolveUpstream(): new (...args: unknown[]) => UpstreamOpenAI {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = requireFromHere("openai") as { default?: unknown } | unknown;
    const candidate =
      (mod as { default?: unknown }).default ?? (mod as unknown);
    if (typeof candidate !== "function") {
      throw new Error(
        "openai did not export a constructor as default or module",
      );
    }
    return candidate as new (...args: unknown[]) => UpstreamOpenAI;
  } catch (err) {
    throw new Error(
      "@actantdb/openai requires `openai` to be installed alongside it. " +
        "Install it with `npm install openai`. " +
        `(underlying error: ${(err as Error).message})`,
    );
  }
}

function summariseMessages(params: {
  messages?: unknown;
  input?: unknown;
  prompt?: unknown;
}): string {
  const msgs = params.messages;
  if (Array.isArray(msgs) && msgs.length > 0) {
    const last = msgs[msgs.length - 1] as { content?: unknown };
    if (last && typeof last === "object" && "content" in last) {
      const c = last.content;
      if (typeof c === "string") return c.slice(0, 200);
      if (Array.isArray(c)) {
        const firstText = c.find(
          (b): b is { type?: string; text?: string } =>
            typeof b === "object" && b !== null,
        );
        if (firstText && typeof firstText.text === "string")
          return firstText.text.slice(0, 200);
      }
    }
  }
  if (typeof params.input === "string") return params.input.slice(0, 200);
  if (typeof params.prompt === "string") return params.prompt.slice(0, 200);
  return "openai.create()";
}

function extractUsage(result: unknown): {
  tokens_in?: number;
  tokens_out?: number;
} {
  if (!result || typeof result !== "object") return {};
  const usage = (
    result as {
      usage?: {
        prompt_tokens?: number;
        completion_tokens?: number;
        input_tokens?: number;
        output_tokens?: number;
      };
    }
  ).usage;
  if (!usage) return {};
  const out: { tokens_in?: number; tokens_out?: number } = {};
  const tokensIn = usage.prompt_tokens ?? usage.input_tokens;
  const tokensOut = usage.completion_tokens ?? usage.output_tokens;
  if (typeof tokensIn === "number") out.tokens_in = tokensIn;
  if (typeof tokensOut === "number") out.tokens_out = tokensOut;
  return out;
}

/** Wrapped `OpenAI` class. */
export class OpenAI {
  readonly upstream: UpstreamOpenAI;
  readonly actant: ActantHandle | undefined;
  readonly activeRun: RunContext | undefined;
  readonly ownsHandle: boolean;

  constructor(options: OpenAIConstructorOptions = {}) {
    const { actant, _upstreamOverride, ...upstreamOpts } = extractActant(options);
    const UpstreamCtor =
      _upstreamOverride ?? (actant?._upstream as
        | (new (...args: unknown[]) => UpstreamOpenAI)
        | undefined) ?? resolveUpstream();
    this.upstream = new UpstreamCtor(upstreamOpts);

    if (actant) {
      if (actant.handle) {
        this.actant = actant.handle;
        this.ownsHandle = false;
      } else {
        this.actant = createActant({
          project: actant.project,
          ...(actant.storeDir !== undefined ? { storeDir: actant.storeDir } : {}),
        });
        this.ownsHandle = true;
      }
      this.activeRun = actant.run;
    } else {
      this.actant = undefined;
      this.activeRun = undefined;
      this.ownsHandle = false;
    }

    return new Proxy(this, {
      get: (target, prop, receiver) => {
        if (prop === "chat") return wrappedChat(target);
        if (prop === "responses") return wrappedResponses(target);
        if (prop in target) return Reflect.get(target, prop, receiver);
        const val = Reflect.get(target.upstream as object, prop);
        return typeof val === "function" ? val.bind(target.upstream) : val;
      },
    });
  }

  close(): void {
    if (this.ownsHandle && this.actant) this.actant.close();
  }
}

function wrappedChat(self: OpenAI): NonNullable<UpstreamOpenAI["chat"]> {
  const chat = self.upstream.chat;
  if (!chat) {
    return {} as NonNullable<UpstreamOpenAI["chat"]>;
  }
  return new Proxy(chat, {
    get: (target, prop) => {
      if (prop !== "completions") {
        const v = Reflect.get(target, prop);
        return typeof v === "function" ? v.bind(target) : v;
      }
      const completions = target.completions;
      if (!completions) return undefined;
      return new Proxy(completions, {
        get: (cTarget, cProp) => {
          if (cProp !== "create") {
            const v = Reflect.get(cTarget, cProp);
            return typeof v === "function" ? v.bind(cTarget) : v;
          }
          return async (...args: unknown[]) =>
            recordCreate(self, args, cTarget.create.bind(cTarget), "chat.completions");
        },
      });
    },
  });
}

function wrappedResponses(self: OpenAI): NonNullable<UpstreamOpenAI["responses"]> {
  const responses = self.upstream.responses;
  if (!responses) {
    return {} as NonNullable<UpstreamOpenAI["responses"]>;
  }
  return new Proxy(responses, {
    get: (target, prop) => {
      if (prop !== "create") {
        const v = Reflect.get(target, prop);
        return typeof v === "function" ? v.bind(target) : v;
      }
      return async (...args: unknown[]) =>
        recordCreate(self, args, target.create.bind(target), "responses");
    },
  });
}

async function recordCreate(
  self: OpenAI,
  args: unknown[],
  upstream: (...a: unknown[]) => Promise<unknown>,
  surface: string,
): Promise<unknown> {
  const params = (args[0] ?? {}) as {
    model?: string;
    messages?: unknown;
    input?: unknown;
    [k: string]: unknown;
  };

  const handle = self.actant;
  if (!handle) return upstream(...args);

  let run = self.activeRun;
  let adHoc = false;
  if (!run) {
    run = handle.startRun({
      meta: { source: "@actantdb/openai", surface },
    });
    adHoc = true;
  }

  const promptHash = sha256OfJSON(params.messages ?? params.input ?? params);
  const summary = summariseMessages(params);
  const t0 = performance.now();
  try {
    const result = await upstream(...args);
    const usage = extractUsage(result);
    const event: ModelCall = {
      model: params.model ?? `openai:${surface}`,
      role: "generator",
      prompt_hash: promptHash,
      summary,
      ...(usage.tokens_in !== undefined ? { tokens_in: usage.tokens_in } : {}),
      ...(usage.tokens_out !== undefined ? { tokens_out: usage.tokens_out } : {}),
    };
    run.recordModelCall(event);
    if (adHoc)
      run.finish({ ok: true, duration_ms: Math.round(performance.now() - t0) });
    return result;
  } catch (err) {
    const event: ModelCall = {
      model: params.model ?? `openai:${surface}`,
      role: "generator",
      prompt_hash: promptHash,
      summary: `ERROR: ${(err as Error).message}`,
    };
    run.recordModelCall(event);
    if (adHoc)
      run.finish({
        ok: false,
        error: (err as Error).message ?? String(err),
        duration_ms: Math.round(performance.now() - t0),
      });
    throw err;
  }
}

function extractActant(options: OpenAIConstructorOptions): {
  actant: ActantClientOptions | undefined;
  _upstreamOverride: (new (...args: unknown[]) => UpstreamOpenAI) | undefined;
  [key: string]: unknown;
} {
  const { actant, _upstream, ...rest } = options as OpenAIConstructorOptions & {
    _upstream?: new (...args: unknown[]) => UpstreamOpenAI;
  };
  return {
    actant: actant,
    _upstreamOverride: _upstream,
    ...rest,
  };
}

export default OpenAI;
