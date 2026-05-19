/**
 * Public types for `@actantdb/workflow`.
 *
 * Shaped to mirror Upstash Workflow's JS surface where it makes sense.
 * Differences are documented in README.md under "Upstash compatibility".
 */

import type { Ledger } from "@actantdb/core";

/** Duration accepted by `ctx.sleep`. Either ms (number) or `"5m"`, `"7d"`, `"100ms"`. */
export type Duration = number | string;

/**
 * What a workflow run can look like from the outside.
 *
 * `running` — a handler invocation is active right now.
 * `suspended` — the run has paused on a sleep / waitForEvent and will resume.
 * `completed` — the workflow returned cleanly.
 * `failed` — a step threw and was not caught.
 * `cancelled` — `ctx.cancel()` ran or `client.cancel()` was called.
 */
export type RunStatus =
  | "running"
  | "suspended"
  | "completed"
  | "failed"
  | "cancelled";

export interface RunResult<T = unknown> {
  runId: string;
  status: RunStatus;
  /** Only present on `completed`. */
  output?: T;
  /** Only present on `suspended` — present when a sleep is pending. */
  retryAfterMs?: number;
  /** Only present on `suspended` — present when waiting on a notify. */
  waitingForEventId?: string;
  /** Only present on `failed`. */
  error?: { message: string; code?: string };
}

/** Headers carried over HTTP between `serve` and `Client`. */
export const WORKFLOW_RUN_ID_HEADER = "x-workflow-run-id";
export const WORKFLOW_STATUS_HEADER = "x-workflow-status";

/** Body shape posted by `client.trigger` and on every resume. */
export interface WorkflowRequestBody<P = unknown> {
  workflowRunId?: string;
  /** Caller-supplied payload — visible at `ctx.payload`. */
  body?: P;
}

/** Options accepted by `serve(handler, opts?)`. */
export interface ServeOptions {
  /** Reuse a ledger you already opened. Wins over `project` / `storeDir`. */
  ledger?: Ledger;
  /** Project id used when opening a fresh ledger. Defaults to `"workflow"`. */
  project?: string;
  /** Override the default `~/.actantdb/<project>/events.sqlite` location. */
  storeDir?: string;
  /**
   * If true and `ledger` was not supplied, open the ledger in-memory.
   * Tests use this; production callers should pass a real `ledger`.
   */
  inMemory?: boolean;
  /**
   * In local mode, when a sleep suspends, schedule a `setTimeout` that
   * re-invokes the handler so the workflow advances without an external
   * scheduler. Defaults to `true`. Set to `false` if you have your own
   * scheduler.
   */
  autoResume?: boolean;
  /**
   * Override `fetch` used inside `ctx.call`. Mainly for tests; defaults to
   * `globalThis.fetch`.
   */
  fetch?: typeof globalThis.fetch;
}

/** Options for `new Client({ ... })`. */
export interface ClientOptions {
  /** Upstash-compat: the QStash token. Unused locally but accepted. */
  token?: string;
  /** Base URL of the workflow handler (the URL you `serve()` on). */
  baseUrl?: string;
  /**
   * Shared ledger — when present, `Client` writes notify / cancel / trigger
   * events directly instead of going over HTTP. This is the local mode that
   * matches the way every other `@actantdb/*` package wires together.
   */
  ledger?: Ledger;
  /** Override `fetch`. Defaults to `globalThis.fetch`. */
  fetch?: typeof globalThis.fetch;
}

/** Args to `client.trigger`. */
export interface TriggerArgs<P = unknown> {
  /** URL of the `serve()` handler. Falls back to `Client({ baseUrl })`. */
  url?: string;
  body?: P;
  headers?: Record<string, string>;
  /** Pre-allocated run id. If omitted, the SDK generates one. */
  workflowRunId?: string;
  /** Compat field. Currently informational only. */
  retries?: number;
}

export interface TriggerResult {
  workflowRunId: string;
}

/** Args to `client.cancel`. */
export interface CancelArgs {
  workflowRunId: string;
}

/** Args to `client.notify`. */
export interface NotifyArgs<D = unknown> {
  /** The event id a `waitForEvent` is listening on. */
  eventId: string;
  /** Optional payload — delivered to the suspended workflow. */
  eventData?: D;
  /**
   * If supplied, only the named run is auto-resumed (the event is still
   * written globally; other runs waiting on the same `eventId` will see
   * it the next time they wake up).
   */
  workflowRunId?: string;
}

/** Args to `ctx.call`. */
export interface CallArgs<B = unknown> {
  url: string;
  method?: "GET" | "POST" | "PUT" | "DELETE" | "PATCH";
  body?: B;
  headers?: Record<string, string>;
}

/** Result of `ctx.call`. */
export interface CallResult<R = unknown> {
  status: number;
  headers: Record<string, string>;
  body: R;
}

/** Options for `ctx.waitForEvent`. */
export interface WaitForEventOptions {
  /** Max time to wait. Defaults to no timeout. */
  timeout?: Duration;
}
