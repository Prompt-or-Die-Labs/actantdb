import { createActant, type ActantHandle } from "@actantdb/core";

export interface WithActantInngestOptions {
  project: string;
  storeDir?: string;
  actant?: ActantHandle;
}

export interface InngestContextLike {
  event?: { name?: string; data?: unknown };
  step?: unknown;
  [key: string]: unknown;
}

export type InngestHandler<C extends InngestContextLike, R> = (ctx: C) => Promise<R> | R;

export interface WrappedInngestHandler<C extends InngestContextLike, R> {
  (ctx: C): Promise<R>;
  readonly actant: ActantHandle;
}

export function withActantInngest<C extends InngestContextLike, R>(
  handler: InngestHandler<C, R>,
  opts: WithActantInngestOptions,
): WrappedInngestHandler<C, R> {
  const actant =
    opts.actant ??
    createActant({
      project: opts.project,
      ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
    });
  const wrapped = async (ctx: C): Promise<R> => {
    const run = actant.startRun({
      meta: {
        adapter: "inngest",
        event: ctx.event ?? null,
      },
    });
    if (ctx.event?.name) run.recordUserMessage(ctx.event.name);
    try {
      const result = await handler(ctx);
      run.recordEffect({ adapter: "inngest", result });
      run.finish({ ok: true });
      return result;
    } catch (err) {
      run.finish({ ok: false, error: err instanceof Error ? err.message : String(err) });
      throw err;
    }
  };
  Object.defineProperty(wrapped, "actant", { value: actant });
  return wrapped as WrappedInngestHandler<C, R>;
}

export const withActant = withActantInngest;
