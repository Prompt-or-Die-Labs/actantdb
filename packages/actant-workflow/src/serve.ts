/**
 * `serve(handler, opts?)` — HTTP handler factory.
 *
 * Returns a `(req: Request) => Promise<Response>` that:
 *   1. Resolves the run id from `x-workflow-run-id` header or request body.
 *   2. If new, generates one and appends `agent_run_started`.
 *   3. Builds a fresh `WorkflowContext` whose state mirrors the on-disk
 *      ledger as of right now.
 *   4. Invokes the user's handler.
 *   5. Catches `WorkflowSuspended` → returns 202 with `{ status, runId,
 *      retryAfterMs?, waitingForEventId? }`. Optionally schedules a
 *      `setTimeout` resume in local mode.
 *   6. Catches `WorkflowCancelled` → returns 200 `{ status: "cancelled" }`.
 *   7. Catches any other error → returns 500 `{ status: "failed", error }`.
 *   8. On clean return → returns 200 `{ status: "completed", output }`.
 *
 * The Web Fetch API shape (Request / Response) makes it a drop-in for
 * Next.js App Router (`export const POST = serve(...)`), Hono, Bun.serve,
 * Deno, Cloudflare Workers, and anything else that speaks fetch.
 */

import { openLedger, ulid, type Ledger } from "@actantdb/core";

import { WorkflowContext, type WorkflowHandler } from "./context.js";
import {
  WorkflowCancelled,
  WorkflowError,
  WorkflowSuspended,
} from "./errors.js";
import { makeState } from "./runner.js";
import {
  WORKFLOW_RUN_ID_HEADER,
  WORKFLOW_STATUS_HEADER,
  type RunResult,
  type RunStatus,
  type ServeOptions,
  type WorkflowRequestBody,
} from "./types.js";

export interface ServedHandler<P = unknown, R = unknown> {
  (req: Request): Promise<Response>;
  /** Direct invocation for tests / in-process callers. */
  invoke(input: InvokeInput<P>): Promise<RunResult<R>>;
  /** The ledger this handler is bound to (useful for tests + clients). */
  ledger: Ledger;
}

export interface InvokeInput<P = unknown> {
  runId?: string;
  body?: P;
  headers?: Record<string, string>;
}

export function serve<P = unknown, R = unknown>(
  handler: WorkflowHandler<P, R>,
  opts: ServeOptions = {},
): ServedHandler<P, R> {
  const ledger =
    opts.ledger ??
    openLedger({
      project: opts.project ?? "workflow",
      ...(opts.storeDir !== undefined ? { storeDir: opts.storeDir } : {}),
      ...(opts.inMemory ? { inMemory: true } : {}),
    });

  const fetchImpl = opts.fetch ?? globalThis.fetch.bind(globalThis);
  const autoResume = opts.autoResume ?? true;

  async function invoke(
    input: InvokeInput<P>,
  ): Promise<RunResult<R>> {
    // Find or assign a run id.
    const runId = input.runId ?? `wfr_${ulid()}`;
    const existing = ledger.query({ runId, limit: 1 });
    if (existing.length === 0) {
      ledger.append({
        kind: "agent_run_started",
        runId,
        payload: {
          workflow: true,
          payload: input.body ?? null,
        },
      });
    }

    const state = makeState(ledger, runId, { fetch: fetchImpl });
    const ctx = new WorkflowContext<P>({
      runId,
      payload: input.body as P,
      ...(input.headers ? { requestHeaders: input.headers } : {}),
      state,
      ledger,
    });

    try {
      const out = await handler(ctx);
      ledger.append({
        kind: "agent_run_finished",
        runId,
        payload: { status: "completed" },
      });
      return { runId, status: "completed", output: out };
    } catch (err) {
      if (err instanceof WorkflowSuspended) {
        scheduleResume(err.retryAfterMs);
        return {
          runId,
          status: "suspended",
          ...(err.retryAfterMs !== undefined
            ? { retryAfterMs: err.retryAfterMs }
            : {}),
          ...(err.waitingForEventId !== undefined
            ? { waitingForEventId: err.waitingForEventId }
            : {}),
        };
      }
      if (err instanceof WorkflowCancelled) {
        ledger.append({
          kind: "agent_run_finished",
          runId,
          payload: { status: "cancelled" },
        });
        return { runId, status: "cancelled" };
      }
      if (err instanceof WorkflowError && err.code === "run_cancelled") {
        // Already cancelled — make sure we surface the finished row exactly once.
        const finished = ledger
          .query({ runId })
          .some(
            (e) =>
              e.kind === "agent_run_finished" &&
              (e.payload as { status?: string })?.status === "cancelled",
          );
        if (!finished) {
          ledger.append({
            kind: "agent_run_finished",
            runId,
            payload: { status: "cancelled" },
          });
        }
        return { runId, status: "cancelled" };
      }
      const e = err as Error;
      ledger.append({
        kind: "agent_run_finished",
        runId,
        payload: {
          status: "failed",
          error: { message: e.message, name: e.name },
        },
      });
      return {
        runId,
        status: "failed",
        error: {
          message: e.message,
          ...(err instanceof WorkflowError ? { code: err.code } : {}),
        },
      };
    }

    function scheduleResume(retryAfterMs?: number): void {
      if (!autoResume) return;
      const delay = Math.max(0, retryAfterMs ?? 0);
      const t = setTimeout(() => {
        // Re-enter the handler with the same run id. Errors are swallowed —
        // the next caller will see them via the ledger.
        const resumeInput: InvokeInput<P> = {
          runId,
          ...(input.body !== undefined ? { body: input.body } : {}),
        };
        invoke(resumeInput).catch(() => {});
      }, delay);
      // Don't keep the event loop alive in CLIs.
      if (typeof t === "object" && t && typeof (t as { unref?: () => void }).unref === "function") {
        (t as { unref: () => void }).unref();
      }
    }
  }

  const fn = async (req: Request): Promise<Response> => {
    let body: WorkflowRequestBody<P> = {};
    const ct = req.headers.get("content-type") ?? "";
    if (ct.includes("application/json")) {
      try {
        body = (await req.json()) as WorkflowRequestBody<P>;
      } catch {
        // Empty / non-JSON body is fine; trigger may send no body.
        body = {};
      }
    } else {
      const text = await req.text();
      if (text) {
        try {
          body = JSON.parse(text) as WorkflowRequestBody<P>;
        } catch {
          body = { body: text as unknown as P };
        }
      }
    }
    const hdrId = req.headers.get(WORKFLOW_RUN_ID_HEADER) ?? undefined;
    const runId = hdrId ?? body.workflowRunId;
    const headers: Record<string, string> = {};
    req.headers.forEach((v, k) => {
      headers[k] = v;
    });
    const result = await invoke({
      ...(runId !== undefined ? { runId } : {}),
      ...(body.body !== undefined ? { body: body.body } : {}),
      headers,
    });
    return toResponse(result);
  };

  return Object.assign(fn, { invoke, ledger });
}

function toResponse<T>(result: RunResult<T>): Response {
  const status = httpStatusFor(result.status);
  return new Response(JSON.stringify(result), {
    status,
    headers: {
      "content-type": "application/json",
      [WORKFLOW_RUN_ID_HEADER]: result.runId,
      [WORKFLOW_STATUS_HEADER]: result.status,
    },
  });
}

function httpStatusFor(s: RunStatus): number {
  switch (s) {
    case "completed":
      return 200;
    case "cancelled":
      return 200;
    case "suspended":
      return 202;
    case "running":
      return 202;
    case "failed":
      return 500;
  }
}
