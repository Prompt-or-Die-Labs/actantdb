/**
 * @actantdb/box — local-first ActantDB Box.
 *
 * Mirrors the Upstash Box SDK surface (https://upstash.com/docs/box) so
 * porting from `@upstash/box` is a one-line import change:
 *
 *   - import { Box } from "@upstash/box"
 *   + import { Box } from "@actantdb/box"
 *
 * Every Box operation is captured in a hash-chained ActantDB ledger, so
 * the full history (file writes, exec, git, agent calls, schedules) is
 * replayable end-to-end. `mode: "cloud"` is contract-only — Phase 2 in
 * docs/CLOUD_ROADMAP.md lights it up.
 */

export { Box, defaultBoxesRoot } from "./box.js";
export { Run } from "./run.js";
export { BoxAgentAPI } from "./agent.js";
export { BoxExecAPI } from "./exec.js";
export { BoxFilesAPI } from "./files.js";
export { BoxGitAPI } from "./git.js";
export { BoxScheduleAPI } from "./schedule.js";

export { BoxError } from "./errors.js";
export type { BoxErrorCode } from "./errors.js";

export type {
  AgentChunk,
  BoxConfig,
  BoxData,
  BoxMetadata,
  BoxStatus,
  ExecChunk,
  FileEntry,
  GitConfig,
  GitStatus,
  PullRequest,
  RunCost,
  RunStatus,
  Schedule,
  Snapshot,
} from "./types.js";

export type { AgentRunInput } from "./agent.js";
export type { ExecOptions } from "./exec.js";

// Upstash-Box-parity harness presets + model enums.
export type { AgentConfig } from "./types.js";
export { Agent } from "./harnesses/types.js";
export type {
  Harness,
  HarnessRunInput,
  HarnessRunResult,
  AgentHarness,
} from "./harnesses/types.js";
export { getHarness, listHarnesses } from "./harnesses/index.js";
export {
  ClaudeCode,
  OpenAICodex,
  OpenRouterModel,
  OpenCodeModel,
  CursorModel,
} from "./models.js";
export type {
  ClaudeCodeModel,
  OpenAICodexModel,
  OpenRouterModelId,
  OpenCodeModelId,
  CursorModelId,
} from "./models.js";
