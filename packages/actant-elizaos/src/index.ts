import { createActant, ulid, type ActantHandle, type RunContext } from "@actantdb/core";
import type { Risk, ToolCallStatus } from "@actantdb/types";

export type JsonLike =
  | null
  | string
  | number
  | boolean
  | JsonLike[]
  | { [key: string]: JsonLike | undefined };

export interface ElizaMessageLike {
  id?: string;
  userId?: string;
  roomId?: string;
  text?: string;
  content?: string | { text?: string; [key: string]: JsonLike | undefined };
  [key: string]: JsonLike | undefined;
}

export interface ElizaActionLike<Args extends readonly unknown[] = readonly unknown[], Result = unknown> {
  name: string;
  description?: string;
  similes?: string[];
  examples?: JsonLike[];
  validate?: (...args: Args) => boolean | Promise<boolean>;
  handler?: (...args: Args) => Result | Promise<Result>;
  [key: string]:
    | JsonLike
    | string[]
    | ((...args: Args) => boolean | Promise<boolean> | Result | Promise<Result>)
    | undefined;
}

export interface WithActantElizaOptions {
  project?: string;
  storeDir?: string;
  actant?: ActantHandle;
  run?: RunContext;
  risk?: Risk;
  autoFinish?: boolean;
}

export type WrappedElizaAction<A extends ElizaActionLike> = A & {
  readonly actant: ActantHandle;
};

export interface ElizaRuntimeLike<A extends ElizaActionLike = ElizaActionLike> {
  actions?: A[];
  [key: string]: JsonLike | A[] | undefined;
}

export interface ElizaProviderResultLike {
  text?: string;
  values?: Record<string, JsonLike>;
  data?: Record<string, JsonLike>;
}

export interface ActantElizaProvider {
  name: "ACTANTDB_LEDGER";
  description: string;
  dynamic: true;
  get: (
    runtime?: ElizaRuntimeLike,
    message?: ElizaMessageLike,
    state?: Record<string, JsonLike>,
  ) => ElizaProviderResultLike | Promise<ElizaProviderResultLike>;
}

export interface ActantElizaPluginOptions extends WithActantElizaOptions {
  actions?: ElizaActionLike[];
}

export interface ActantElizaPlugin {
  name: "actantdb";
  description: string;
  actions: ElizaActionLike[];
  providers: ActantElizaProvider[];
  actant: ActantHandle;
}

export function withActantElizaAction<A extends ElizaActionLike>(
  action: A,
  opts: WithActantElizaOptions,
): WrappedElizaAction<A> {
  const actant = resolveActant(opts);
  if (!action.handler) {
    return withActantProperty({ ...action } as A, actant);
  }
  const original = action.handler;
  const wrapped = {
    ...action,
    async handler(...args: Parameters<NonNullable<A["handler"]>>) {
      const ownedRun = opts.run === undefined;
      const run =
        opts.run ??
        actant.startRun({
          meta: {
            adapter: "elizaos",
            action: action.name,
          },
        });
      const text = extractMessageText(args[1]) ?? extractMessageText(args[0]);
      if (text) run.recordUserMessage(text);
      const toolCallId = ulid();
      const started = Date.now();
      const recordedArgs = argsToJSON(args);
      run.recordToolCallRequested({
        tool_call_id: toolCallId,
        tool: action.name,
        args: recordedArgs,
        risk: opts.risk ?? "low",
      });
      run.recordToolCallStarted(toolCallId, recordedArgs);
      try {
        const result = await original(...args);
        recordCompletion(run, toolCallId, "ok", result, started);
        if (ownedRun && opts.autoFinish !== false) run.finish({ ok: true });
        return result;
      } catch (error) {
        recordCompletion(run, toolCallId, "error", errorToJSON(error), started);
        if (ownedRun && opts.autoFinish !== false) {
          run.finish({ ok: false, error: error instanceof Error ? error.message : String(error) });
        }
        throw error;
      }
    },
  } as A;
  return withActantProperty(wrapped, actant);
}

export function withActantElizaRuntime<R extends ElizaRuntimeLike>(
  runtime: R,
  opts: WithActantElizaOptions,
): R & { readonly actant: ActantHandle } {
  const actant = resolveActant(opts);
  const wrappedActions = runtime.actions?.map((action) =>
    withActantElizaAction(action, { ...opts, actant }),
  );
  return withActantProperty(
    {
      ...runtime,
      ...(wrappedActions !== undefined ? { actions: wrappedActions } : {}),
    } as R,
    actant,
  );
}

export function createActantElizaPlugin(opts: ActantElizaPluginOptions): ActantElizaPlugin {
  const actant = resolveActant(opts);
  const actions = (opts.actions ?? []).map((action) =>
    withActantElizaAction(action, { ...opts, actant }),
  );
  return {
    name: "actantdb",
    description: "Records elizaOS action execution into an ActantDB ledger.",
    actions,
    providers: [
      {
        name: "ACTANTDB_LEDGER",
        description: "Current ActantDB ledger path for this elizaOS runtime.",
        dynamic: true,
        get: () => ({
          text: `actantdb project=${actant.project}`,
          values: {
            project: actant.project,
            db: actant.ledger.path(),
          },
          data: {
            project: actant.project,
            db: actant.ledger.path(),
          },
        }),
      },
    ],
    actant,
  };
}

export const withActant = withActantElizaAction;

function resolveActant(opts: WithActantElizaOptions): ActantHandle {
  if (opts.actant) return opts.actant;
  if (!opts.project) {
    throw new Error("@actantdb/elizaos requires either opts.actant or opts.project");
  }
  return createActant({
    project: opts.project,
    ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
  });
}

function withActantProperty<T extends object>(value: T, actant: ActantHandle): T & { readonly actant: ActantHandle } {
  Object.defineProperty(value, "actant", { value: actant });
  return value as T & { readonly actant: ActantHandle };
}

function extractMessageText(value: unknown): string | undefined {
  if (!isRecord(value)) return undefined;
  if (typeof value.text === "string") return value.text;
  const content = value.content;
  if (typeof content === "string") return content;
  if (isRecord(content) && typeof content.text === "string") return content.text;
  return undefined;
}

function recordCompletion(
  run: RunContext,
  toolCallId: string,
  status: ToolCallStatus,
  result: unknown,
  started: number,
): void {
  run.recordToolCallCompleted({
    tool_call_id: toolCallId,
    status,
    result,
    duration_ms: Date.now() - started,
  });
}

function argsToJSON(args: readonly unknown[]): unknown {
  return args.map(toSerializable);
}

function errorToJSON(error: unknown): unknown {
  if (error instanceof Error) {
    return { name: error.name, message: error.message };
  }
  return { message: String(error) };
}

function toSerializable(value: unknown): unknown {
  if (value === null) return null;
  const type = typeof value;
  if (type === "string" || type === "number" || type === "boolean") return value;
  if (Array.isArray(value)) return value.map(toSerializable);
  if (isRecord(value)) {
    const out: Record<string, unknown> = {};
    for (const [key, entry] of Object.entries(value)) {
      if (typeof entry !== "function") out[key] = toSerializable(entry);
    }
    return out;
  }
  return String(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
