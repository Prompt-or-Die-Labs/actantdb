/**
 * @actantdb/anthropic — drop-in replacement for `@anthropic-ai/sdk` that
 * records every `messages.create()` call as a `model_call` event in the
 * ActantDB ledger.
 *
 *   import Anthropic from "@actantdb/anthropic";
 *
 *   const client = new Anthropic({
 *     apiKey: process.env.ANTHROPIC_API_KEY,
 *     actant: { project: "my-app", storeDir: "./.actantdb" },
 *   });
 *
 *   const msg = await client.messages.create({ model: "...", ... });
 *
 * `msg` is the upstream response, byte-for-byte. A `model_call` event is
 * also written. If `actant` is omitted, the client is a transparent
 * passthrough.
 *
 * Implementation notes:
 *   - The upstream `@anthropic-ai/sdk` is an OPTIONAL peer dep. We resolve
 *     it lazily via `createRequire` so this package builds & runs even
 *     when the peer isn't installed (e.g. in CI without API keys).
 *   - We use a Proxy so every other property / method / sub-client is
 *     forwarded verbatim — no need to mirror the upstream surface.
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
  /** Project identifier used for the local store. */
  project: string;
  /** Override storage root (default: ~/.actantdb). */
  storeDir?: string;
  /** Optional explicit ActantDB handle. If provided, `project`/`storeDir`
   *  are ignored and this handle is used directly. */
  handle?: ActantHandle;
  /** Optional active run context. If omitted, each `.create()` opens an
   *  ad-hoc single-event run and finishes it immediately. */
  run?: RunContext;
  /** Test-only escape hatch: inject the upstream class. When set, we use
   *  this constructor instead of resolving `@anthropic-ai/sdk`. */
  _upstream?: new (...args: unknown[]) => unknown;
}

/** Constructor options accepted by the wrapper: anything the upstream
 *  accepts, plus an optional `actant` block. */
export interface AnthropicConstructorOptions {
  /** Anything the upstream `Anthropic` class accepts. */
  [key: string]: unknown;
  /** ActantDB capture options. Omit for a transparent passthrough. */
  actant?: ActantClientOptions;
}

interface UpstreamAnthropic {
  messages: {
    create: (...args: unknown[]) => Promise<unknown>;
  };
  [key: string]: unknown;
}

function resolveUpstream(): new (...args: unknown[]) => UpstreamAnthropic {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = requireFromHere("@anthropic-ai/sdk") as
      | { default?: unknown }
      | unknown;
    const candidate =
      (mod as { default?: unknown }).default ?? (mod as unknown);
    if (typeof candidate !== "function") {
      throw new Error(
        "@anthropic-ai/sdk did not export a constructor as default or module",
      );
    }
    return candidate as new (...args: unknown[]) => UpstreamAnthropic;
  } catch (err) {
    throw new Error(
      "@actantdb/anthropic requires `@anthropic-ai/sdk` to be installed " +
        "alongside it. Install it with `npm install @anthropic-ai/sdk`. " +
        `(underlying error: ${(err as Error).message})`,
    );
  }
}

function summariseMessages(messages: unknown): string {
  if (!Array.isArray(messages) || messages.length === 0) return "messages.create()";
  const last = messages[messages.length - 1] as { content?: unknown };
  if (last && typeof last === "object" && "content" in last) {
    const c = last.content;
    if (typeof c === "string") return c.slice(0, 200);
    if (Array.isArray(c)) {
      const firstText = c.find(
        (b): b is { type: string; text: string } =>
          typeof b === "object" && b !== null && (b as { type?: unknown }).type === "text",
      );
      if (firstText) return String(firstText.text).slice(0, 200);
    }
  }
  return "messages.create()";
}

/** Wrapped `Anthropic` class. Constructor accepts `{ ...upstreamOpts, actant }`. */
export class Anthropic {
  /** The underlying upstream client. Always present. */
  readonly upstream: UpstreamAnthropic;
  /** ActantDB handle, if capture was enabled. */
  readonly actant: ActantHandle | undefined;
  /** Active run context (provided by caller or ad-hoc per call). */
  readonly activeRun: RunContext | undefined;
  /** Whether this client manages its own handle (and should close it). */
  readonly ownsHandle: boolean;

  constructor(options: AnthropicConstructorOptions = {}) {
    const { actant, _upstreamOverride, ...upstreamOpts } = extractActant(options);
    const UpstreamCtor =
      _upstreamOverride ?? (actant?._upstream as
        | (new (...args: unknown[]) => UpstreamAnthropic)
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
        if (prop === "messages") return wrappedMessages(target);
        if (prop in target) return Reflect.get(target, prop, receiver);
        // Forward any other property to the upstream client.
        const val = Reflect.get(target.upstream as object, prop);
        return typeof val === "function" ? val.bind(target.upstream) : val;
      },
    });
  }

  /** Close the underlying ledger if this client created it. */
  close(): void {
    if (this.ownsHandle && this.actant) this.actant.close();
  }
}

function wrappedMessages(self: Anthropic): UpstreamAnthropic["messages"] {
  const upstreamMessages = self.upstream.messages;
  return new Proxy(upstreamMessages, {
    get: (target, prop) => {
      if (prop !== "create") {
        const v = Reflect.get(target, prop);
        return typeof v === "function" ? v.bind(target) : v;
      }
      return async (...args: unknown[]) => recordedCreate(self, args);
    },
  });
}

async function recordedCreate(self: Anthropic, args: unknown[]): Promise<unknown> {
  const params = (args[0] ?? {}) as {
    model?: string;
    messages?: unknown;
    [k: string]: unknown;
  };

  const handle = self.actant;
  if (!handle) {
    return self.upstream.messages.create(...args);
  }

  // Either reuse the caller's run or open an ad-hoc one for this call.
  let run = self.activeRun;
  let adHoc = false;
  if (!run) {
    run = handle.startRun({ meta: { source: "@actantdb/anthropic" } });
    adHoc = true;
  }

  const promptHash = sha256OfJSON(params.messages ?? params);
  const summary = summariseMessages(params.messages);
  const t0 = performance.now();
  try {
    const result = await self.upstream.messages.create(...args);
    const usage = extractModelUsage(result);
    const event: ModelCall = {
      model: params.model ?? "anthropic:unknown",
      role: "generator",
      prompt_hash: promptHash,
      summary,
      ...(usage.tokens_in !== undefined ? { tokens_in: usage.tokens_in } : {}),
      ...(usage.tokens_out !== undefined ? { tokens_out: usage.tokens_out } : {}),
    };
    run.recordModelCall(event);
    if (adHoc) run.finish({ ok: true, duration_ms: Math.round(performance.now() - t0) });
    return result;
  } catch (err) {
    const event: ModelCall = {
      model: params.model ?? "anthropic:unknown",
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

function extractActant(options: AnthropicConstructorOptions): {
  actant: ActantClientOptions | undefined;
  _upstreamOverride: (new (...args: unknown[]) => UpstreamAnthropic) | undefined;
  [key: string]: unknown;
} {
  const { actant, _upstream, ...rest } = options as AnthropicConstructorOptions & {
    _upstream?: new (...args: unknown[]) => UpstreamAnthropic;
  };
  return {
    actant: actant,
    _upstreamOverride: _upstream,
    ...rest,
  };
}

function extractModelUsage(
  result: unknown,
): { tokens_in?: number; tokens_out?: number } {
  if (!result || typeof result !== "object") return {};
  const usage = (result as { usage?: { input_tokens?: number; output_tokens?: number } }).usage;
  if (!usage) return {};
  const out: { tokens_in?: number; tokens_out?: number } = {};
  if (typeof usage.input_tokens === "number") out.tokens_in = usage.input_tokens;
  if (typeof usage.output_tokens === "number") out.tokens_out = usage.output_tokens;
  return out;
}

export default Anthropic;
