// ActantDB — umbrella export. Single import, everything in one place.
//
// Minimal usage:
//
//   import { openLedger, evaluate, withActant } from "actantdb";
//
//   const ledger = openLedger("my-project");
//   const wrapped = withActant(myAgent, { ledger });
//   const verdict = evaluate(myPolicy, toolCall);
//
// If you only need one piece, prefer the individual package — same code,
// smaller install:
//
//   import { openLedger } from "@actantdb/core";
//   import { evaluate }   from "@actantdb/policy";
//   import { withActant } from "@actantdb/mastra";
//
// All exports here are re-exports — no new types, no new behavior.
export * from "@actantdb/core";
export * from "@actantdb/policy";
export * from "@actantdb/mastra";
export * from "@actantdb/replay";
export * from "@actantdb/sdk";
export * from "@actantdb/box";
// Framework adapters whose named exports don't collide with anything above.
// The Anthropic / OpenAI default-export classes collide with each other so
// they're reachable only via subpaths (`actantdb/anthropic`, `actantdb/openai`).
export { wrapAiSdk } from "@actantdb/ai-sdk";
export type {
  AiSdkLike,
  AiSdkToolLike,
  AiSdkCallParams,
  WrapAiSdkOptions,
  WrappedAiSdk,
} from "@actantdb/ai-sdk";
export { withActantAgent } from "@actantdb/openai-agents";
export type {
  OpenAIAgentLike,
  OpenAIAgentToolLike,
  WithActantAgentOptions,
  WrappedOpenAIAgent,
} from "@actantdb/openai-agents";
export { ActantCallbackHandler } from "@actantdb/langchain";
export type { ActantCallbackHandlerOptions } from "@actantdb/langchain";
// @actantdb/workflow re-exported selectively to avoid name collisions:
//   - `ClientOptions` collides with @actantdb/sdk's ClientOptions
//   - `RunStatus` collides with @actantdb/box's RunStatus
//   - `RunResult` is a deprecated alias we don't want in the umbrella surface
// Use `import { ... } from "actantdb/workflow"` (or "@actantdb/workflow") for the full surface.
export {
  serve,
  Client,
  WorkflowContext,
  parseDuration,
  parseAbsoluteTime,
  WorkflowError,
  WorkflowSuspended,
  WorkflowCancelled,
  WORKFLOW_RUN_ID_HEADER,
  WORKFLOW_STATUS_HEADER,
} from "@actantdb/workflow";
export type {
  ServedHandler,
  InvokeInput,
  WorkflowHandler,
  WorkflowErrorCode,
  SuspensionReason,
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
} from "@actantdb/workflow";
export type * from "@actantdb/types";
