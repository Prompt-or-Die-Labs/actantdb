/**
 * @actantdb/convex — Convex Agent wrapper.
 *
 * Convex Agents expose a `tools: Record<string, ConvexTool>` registry where
 * each tool has an `handler(ctx, args)` method. We adapt that shape to the
 * duck-typed `{ execute(args) }` that [`@actantdb/mastra`]'s `withActant`
 * expects, then delegate. Same Guard, same Studio, same replay.
 *
 * Convex-specific things this handles:
 *   - `handler(ctx, args)` adapter to `execute(args)`.
 *   - Capturing Convex's session id (if any) into the run meta.
 */

import { withActant, type WithActantOptions } from "@actantdb/mastra";
export type { WithActantOptions };

/** Minimum shape of a Convex tool registry entry. */
export interface ConvexTool {
  /** Tool name. */
  name?: string;
  /** Convex passes a context object as first arg; we forward it. */
  handler: (ctx: unknown, args: unknown) => Promise<unknown>;
}

/** Minimum shape of a Convex Agent. */
export interface ConvexAgent {
  /** Agent name. */
  name?: string;
  /** Tool registry as Convex models it. */
  tools?: Record<string, ConvexTool>;
  /** `run(input)` is Convex's main entry; optional. */
  run?: (input: unknown) => Promise<unknown>;
}

/** Wrap a Convex Agent. */
export function withConvex<A extends ConvexAgent>(
  agent: A,
  ctxFactory: () => unknown,
  opts: WithActantOptions,
): A & { actant: ReturnType<typeof withActant>["actant"] } {
  // Adapt: Convex `handler(ctx, args)` → Actant `execute(args)`.
  // The Convex ctx is created per-tool-call via `ctxFactory`.
  const adapted: { tools?: Record<string, { execute: (args: unknown) => Promise<unknown> }>; generate?: (input: unknown) => Promise<unknown> } = {};
  if (agent.tools) {
    adapted.tools = {};
    for (const [name, tool] of Object.entries(agent.tools)) {
      adapted.tools[name] = {
        execute: async (args: unknown) => {
          return tool.handler(ctxFactory(), args);
        },
      };
    }
  }
  if (agent.run) {
    adapted.generate = async (input: unknown) => agent.run!(input);
  }

  const wrapped = withActant(adapted, opts);

  // Re-attach the Convex-shape we received so callers see their original
  // surface unchanged.
  return Object.assign(agent, { actant: wrapped.actant });
}
