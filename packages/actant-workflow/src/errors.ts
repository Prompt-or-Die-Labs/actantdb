/**
 * Errors raised by `@actantdb/workflow`.
 *
 * `WorkflowError` is the public surface for "something went wrong while
 * running the workflow" — bad config, missing run, duplicate step name,
 * cancelled run, etc.
 *
 * `WorkflowSuspended` is an internal control-flow sentinel — NOT a real
 * error from the caller's perspective. It bubbles out of the workflow
 * function to tell the runner "I hit a non-ready sleep / waitForEvent;
 * pause this invocation, the next resume will pick up where I stopped".
 * The runner is the only thing that should catch it.
 */

export class WorkflowError extends Error {
  readonly code: WorkflowErrorCode;
  readonly cause?: unknown;
  constructor(code: WorkflowErrorCode, message: string, cause?: unknown) {
    super(message);
    this.name = "WorkflowError";
    this.code = code;
    if (cause !== undefined) this.cause = cause;
  }
}

export type WorkflowErrorCode =
  | "duplicate_step"
  | "run_cancelled"
  | "not_found"
  | "invalid_duration"
  | "invalid_request"
  | "step_threw";

/**
 * Internal sentinel: the workflow function reached a sleep / waitForEvent
 * that is not yet ready. The runner catches this and tells the HTTP
 * handler to return a 202 "suspended" response.
 *
 * Carries the kind of suspension and (optionally) the duration we need to
 * wait so a local-mode runner can schedule a `setTimeout` to retrigger the
 * handler.
 */
export class WorkflowSuspended extends Error {
  readonly reason: SuspensionReason;
  /** ms until the suspension is expected to clear (sleep / sleepUntil only). */
  readonly retryAfterMs?: number;
  /** event id we're waiting for (waitForEvent only). */
  readonly waitingForEventId?: string;
  /** name of the step that suspended — for debugging / logs. */
  readonly stepName: string;
  constructor(
    reason: SuspensionReason,
    stepName: string,
    opts: { retryAfterMs?: number; waitingForEventId?: string } = {},
  ) {
    super(`workflow suspended: ${reason} @ ${stepName}`);
    this.name = "WorkflowSuspended";
    this.reason = reason;
    this.stepName = stepName;
    if (opts.retryAfterMs !== undefined) this.retryAfterMs = opts.retryAfterMs;
    if (opts.waitingForEventId !== undefined)
      this.waitingForEventId = opts.waitingForEventId;
  }
}

export type SuspensionReason = "sleep" | "wait_for_event";

/**
 * Internal sentinel: `ctx.cancel()` was called from inside a workflow.
 * The runner catches it, writes the cancel marker, and finishes the run.
 */
export class WorkflowCancelled extends Error {
  constructor() {
    super("workflow cancelled");
    this.name = "WorkflowCancelled";
  }
}
