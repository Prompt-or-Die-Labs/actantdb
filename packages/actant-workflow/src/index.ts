/**
 * `@actantdb/workflow` — drop-in port of Upstash Workflow on top of the
 * ActantDB ledger.
 *
 * Replace `import { serve, Client } from "@upstash/workflow"` with
 * `import { serve, Client } from "@actantdb/workflow"` and your existing
 * workflow code keeps working. Persistence moves from QStash to the
 * local ledger; everything else stays the same.
 *
 * See README.md for the full API + migration notes.
 */

export { serve, type ServedHandler, type InvokeInput } from "./serve.js";
export { Client } from "./client.js";
export {
  WorkflowContext,
  type WorkflowHandler,
} from "./context.js";

export { parseDuration, parseAbsoluteTime } from "./duration.js";

export {
  WorkflowError,
  WorkflowSuspended,
  WorkflowCancelled,
  type WorkflowErrorCode,
  type SuspensionReason,
} from "./errors.js";

export type {
  Duration,
  WorkflowRunStatus,
  WorkflowRunResult,
  ServeOptions,
  WorkflowClientOptions,
  TriggerArgs,
  TriggerResult,
  CancelArgs,
  NotifyArgs,
  CallArgs,
  CallResult,
  WaitForEventOptions,
  WorkflowRequestBody,
  // Deprecated short aliases — re-exported for direct-import callers, but
  // the umbrella @actantdb/all should NOT pull these in (it has collisions
  // with @actantdb/sdk's ClientOptions and @actantdb/box's RunStatus).
  RunStatus,
  RunResult,
  ClientOptions,
} from "./types.js";

export {
  WORKFLOW_RUN_ID_HEADER,
  WORKFLOW_STATUS_HEADER,
} from "./types.js";
