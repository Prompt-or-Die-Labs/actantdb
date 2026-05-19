/**
 * @actantdb/box — public types.
 *
 * Surface mirrors Upstash Box (https://upstash.com/docs/box) so consumer code
 * is portable. Where Upstash carries cloud-only concepts (apiKey, host) we
 * accept them but they are no-ops in local mode.
 */

import type { MastraAgentLike } from "@actantdb/mastra";

/** Configuration passed to `Box.create`. */
export interface BoxConfig {
  /** Display name. Defaults to the box id. */
  name?: string;
  /** "local" runs on the host (default). "cloud" throws — see CLOUD_ROADMAP.md. */
  mode?: "local" | "cloud";
  /**
   * Pre-existing agent the Box will run. The agent must conform to the
   * `MastraAgentLike` shape (a `tools` record and an optional `generate()`).
   * If omitted you can still use `box.exec`/`box.files`/etc., but
   * `box.agent.run` will throw.
   */
  agent?: MastraAgentLike;
  /** Override the ~/.actantdb/boxes root. */
  storeRoot?: string;
  /** Initial cwd relative to the workspace dir. Default: "". */
  cwd?: string;
  /** Optional ledger project id. Defaults to the box id. */
  project?: string;
  /** Optional model identifier (display only — Box doesn't call a model). */
  model?: string;
  /** Optional command run on Box creation (e.g. "git clone <repo>"). */
  initCommand?: string;
  /** Reserved for cloud mode. Ignored in local mode. */
  apiKey?: string;
  /** If true (default), the Box stays resident after `await Box.create()` resolves. */
  keepAlive?: boolean;
}

/** Lightweight metadata for `Box.list`. */
export interface BoxData {
  id: string;
  name: string;
  createdAt: string;
  workspaceDir: string;
  status: BoxStatus;
  mode: "local" | "cloud";
  model?: string;
}

export type BoxStatus = "running" | "paused" | "deleted";

/** Returned by `box.files.list`. */
export interface FileEntry {
  /** Relative path from the workspace root. */
  path: string;
  /** Absolute path on disk. */
  absolutePath: string;
  /** "file" | "directory" | "other". */
  kind: "file" | "directory" | "other";
  /** Size in bytes (files only; 0 for directories). */
  size: number;
}

/** Returned by `box.git.status`. */
export interface GitStatus {
  branch: string;
  ahead: number;
  behind: number;
  /** Files with pending changes. */
  files: { path: string; status: string }[];
  clean: boolean;
}

/** Returned by `box.git.updateConfig` / inspected via `box.git`. */
export interface GitConfig {
  userName?: string;
  userEmail?: string;
}

/** Returned by `box.git.createPR`. */
export interface PullRequest {
  url: string;
  number?: number;
  /** True if the request was actually submitted; false if `gh` was missing
   *  and we fell back to printing the command. */
  submitted: boolean;
  command: string;
}

/** Schedule descriptor. */
export interface Schedule {
  id: string;
  kind: "exec" | "agent";
  cron?: string;
  everyMs?: number;
  command?: string;
  prompt?: string;
  timeoutMs?: number;
  status: "active" | "paused";
  createdAt: string;
  lastFiredAt?: string;
  runs: number;
}

/** Snapshot metadata. */
export interface Snapshot {
  id: string;
  name?: string;
  boxId: string;
  createdAt: string;
  /** Path to the tar file. */
  archivePath: string;
  /** Anchored ledger event id (most recent at snapshot time, if any). */
  anchorEventId?: string;
}

/** Cost breakdown attached to every Run. */
export interface RunCost {
  inputTokens: number;
  outputTokens: number;
  computeMs: number;
  totalUsd: number;
}

export type RunStatus = "pending" | "running" | "ok" | "error" | "cancelled";

/** Streaming chunk from `box.agent.stream`. */
export type AgentChunk =
  | { type: "text-delta"; text: string }
  | { type: "tool-call"; toolName: string; input: unknown }
  | { type: "tool-result"; toolName: string; result: unknown }
  | { type: "finish"; result: unknown };

/** Streaming chunk from `box.exec.stream`. */
export type ExecChunk =
  | { type: "stdout"; line: string }
  | { type: "stderr"; line: string }
  | { type: "exit"; code: number | null };

/** Persisted box.json metadata (written to <workspace>/.actantdb/box.json). */
export interface BoxMetadata {
  id: string;
  name: string;
  createdAt: string;
  mode: "local" | "cloud";
  storeRoot: string;
  workspaceDir: string;
  cwd: string;
  project: string;
  model?: string;
  initCommand?: string;
  keepAlive: boolean;
  status: BoxStatus;
}
