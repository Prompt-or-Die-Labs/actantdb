/**
 * Internal harness contract.
 *
 * A harness drives a coding-agent CLI installed on the host (or in the
 * cloud container, once Phase 2 lands). The Box's `box.agent.run` /
 * `box.agent.stream` calls dispatch to the configured harness instead of
 * the user-supplied `agent` object when `config.agent.harness` is set.
 */

import type { Run } from "../run.js";
import type { AgentChunk } from "../types.js";

export interface HarnessRunInput {
  prompt: string;
  /** Workspace directory the CLI runs inside. */
  cwd: string;
  /** Model id (`anthropic/...`, `openai/...`, etc). */
  model?: string;
  /** API key forwarded to the CLI via env var (provider-specific). */
  apiKey?: string;
  /** Soft timeout — the harness wraps `spawn` in a Promise.race. */
  timeoutMs?: number;
  /** Any extra args appended to the spawn command. */
  extraArgs?: string[];
}

export interface HarnessRunResult {
  ok: boolean;
  output: string;
  /** Final stdout chunk, parsed if the CLI emits JSON; otherwise raw text. */
  result: unknown;
  /** Wall-clock duration. */
  computeMs: number;
  /** Exit code (non-null when the process exited normally). */
  exitCode: number | null;
}

/** A harness adapter. Each preset (ClaudeCode / Codex / OpenCode) implements this. */
export interface Harness {
  /** Stable name, e.g. "claude-code". */
  readonly name: string;

  /** Default model id. */
  readonly defaultModel: string;

  /** Default env-var the CLI reads for its API key. */
  readonly apiKeyEnv: string;

  /** Filesystem command we spawn. Override via env var (see findCli). */
  readonly cliName: string;

  /** Run once, return the final result. */
  run(input: HarnessRunInput): Promise<HarnessRunResult>;

  /** Stream chunks. Implementations parse stdout into AgentChunk shapes. */
  stream(input: HarnessRunInput): AsyncIterable<AgentChunk>;
}

/** Agent harness presets — string enum so consumer code reads as a name. */
export const Agent = {
  ClaudeCode: "claude-code",
  Codex: "codex",
  OpenCode: "opencode",
  Cursor: "cursor",
  /** Sentinel meaning "use the user-supplied `agent` object verbatim". */
  Custom: "custom",
} as const;
export type AgentHarness = (typeof Agent)[keyof typeof Agent];
