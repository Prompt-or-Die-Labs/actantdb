import { createActant, type ActantHandle } from "@actantdb/core";

export interface WithActantTriggerOptions {
  project: string;
  storeDir?: string;
  actant?: ActantHandle;
}

export interface TriggerTaskContextLike {
  task?: { id?: string; name?: string };
  run?: { id?: string };
  payload?: unknown;
  [key: string]: unknown;
}

export type TriggerTaskHandler<C extends TriggerTaskContextLike, R> = (ctx: C) => Promise<R> | R;

export interface WrappedTriggerTask<C extends TriggerTaskContextLike, R> {
  (ctx: C): Promise<R>;
  readonly actant: ActantHandle;
}

export function withActantTriggerTask<C extends TriggerTaskContextLike, R>(
  handler: TriggerTaskHandler<C, R>,
  opts: WithActantTriggerOptions,
): WrappedTriggerTask<C, R> {
  const actant =
    opts.actant ??
    createActant({
      project: opts.project,
      ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
    });
  const wrapped = async (ctx: C): Promise<R> => {
    const run = actant.startRun({
      meta: {
        adapter: "triggerdev",
        task: ctx.task ?? null,
        triggerRun: ctx.run ?? null,
      },
    });
    if (ctx.task?.name) run.recordUserMessage(ctx.task.name);
    try {
      const result = await handler(ctx);
      run.recordEffect({ adapter: "triggerdev", result });
      run.finish({ ok: true });
      return result;
    } catch (err) {
      run.finish({ ok: false, error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  };
  Object.defineProperty(wrapped, "actant", { value: actant });
  return wrapped as WrappedTriggerTask<C, R>;
}

export const withActant = withActantTriggerTask;
