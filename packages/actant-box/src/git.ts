/**
 * @actantdb/box — git namespace.
 *
 * Thin wrapper around the `git` CLI executed inside the box workspace. Every
 * call records ledger events under tool name `git.<verb>`.
 *
 * `createPR` shells out to `gh` if present; otherwise it returns the command
 * the user would need to run by hand (submitted=false).
 */

import { spawn } from "node:child_process";
import { join } from "node:path";

import { ulid, type Ledger } from "@actantdb/core";

import { BoxError } from "./errors.js";
import type { GitConfig, GitStatus, PullRequest } from "./types.js";

export interface GitCtx {
  ledger: Ledger;
  workspaceDir: string;
  cwd: string;
}
type CtxProvider = () => GitCtx;

export class BoxGitAPI {
  constructor(private readonly ctx: CtxProvider) {}

  async clone(opts: { repo: string; branch?: string; dir?: string }): Promise<void> {
    const args = ["clone"];
    if (opts.branch) args.push("-b", opts.branch);
    args.push(opts.repo);
    if (opts.dir) args.push(opts.dir);
    await this.exec({ args });
  }

  async diff(): Promise<string> {
    const { output } = await this.exec({ args: ["diff"] });
    return output;
  }

  async status(): Promise<GitStatus> {
    const { output: branchOut } = await this.execSilent({
      args: ["rev-parse", "--abbrev-ref", "HEAD"],
    }).catch(() => ({ output: "HEAD" }));
    const branch = branchOut.trim();

    const { output: portcelain } = await this.execSilent({ args: ["status", "--porcelain"] });
    const files = portcelain
      .split("\n")
      .filter((l) => l.length > 0)
      .map((l) => {
        const status = l.slice(0, 2).trim();
        const path = l.slice(3);
        return { status, path };
      });

    let ahead = 0;
    let behind = 0;
    try {
      const { output: counts } = await this.execSilent({
        args: ["rev-list", "--left-right", "--count", "@{u}...HEAD"],
      });
      const m = counts.trim().split(/\s+/);
      if (m.length === 2 && m[0] !== undefined && m[1] !== undefined) {
        behind = Number(m[0]) || 0;
        ahead = Number(m[1]) || 0;
      }
    } catch {
      // no upstream tracked
    }
    return { branch, ahead, behind, files, clean: files.length === 0 };
  }

  async commit(opts: {
    message: string;
    authorName?: string;
    authorEmail?: string;
  }): Promise<void> {
    if (opts.authorName || opts.authorEmail) {
      await this.updateConfig({
        ...(opts.authorName !== undefined ? { userName: opts.authorName } : {}),
        ...(opts.authorEmail !== undefined ? { userEmail: opts.authorEmail } : {}),
      });
    }
    await this.exec({ args: ["add", "-A"] });
    await this.exec({ args: ["commit", "-m", opts.message] });
  }

  async updateConfig(cfg: GitConfig): Promise<GitConfig> {
    if (cfg.userName !== undefined) await this.exec({ args: ["config", "user.name", cfg.userName] });
    if (cfg.userEmail !== undefined) await this.exec({ args: ["config", "user.email", cfg.userEmail] });
    return cfg;
  }

  async push(opts: { branch?: string; remote?: string } = {}): Promise<void> {
    const args = ["push"];
    if (opts.remote) args.push(opts.remote);
    if (opts.branch) args.push(opts.branch);
    await this.exec({ args });
  }

  async createPR(opts: { title: string; body?: string; base?: string }): Promise<PullRequest> {
    const ghArgs = ["pr", "create", "--title", opts.title];
    if (opts.body) ghArgs.push("--body", opts.body);
    if (opts.base) ghArgs.push("--base", opts.base);
    const command = `gh ${ghArgs.map((a) => (a.includes(" ") ? `"${a}"` : a)).join(" ")}`;
    const ghAvailable = await hasBinary("gh");
    if (!ghAvailable) {
      this.recordEffect("pr_fallback", { command, reason: "gh_missing" });
      return { url: "", submitted: false, command };
    }
    const originRemote = await this.hasOriginRemote();
    if (!originRemote) {
      this.recordEffect("pr_fallback", { command, reason: "origin_remote_missing" });
      return { url: "", submitted: false, command };
    }
    const { output } = await runProcess(
      "gh",
      ghArgs,
      this.workspaceCwd(),
      {
        ...process.env,
        GH_PROMPT_DISABLED: "1",
      },
      { timeoutMs: 30_000 },
    );
    const urlMatch = output.match(/https?:\/\/\S+/);
    const url = urlMatch ? urlMatch[0] : "";
    this.recordEffect("pr_created", { url, command });
    return { url, submitted: true, command };
  }

  async exec(opts: { args: string[] }): Promise<{ output: string }> {
    const { ledger } = this.ctx();
    const runId = `git-${ulid()}`;
    const toolCallId = ulid();
    const t0 = performance.now();
    ledger.append({
      kind: "tool_call_requested",
      runId,
      payload: {
        tool_call_id: toolCallId,
        tool: `git.${opts.args[0] ?? "exec"}`,
        args: { args: opts.args },
        risk: "medium",
      },
      sensitivity: "low",
    });
    ledger.append({
      kind: "tool_call_started",
      runId,
      payload: { tool_call_id: toolCallId, final_args: { args: opts.args } },
      sensitivity: "low",
    });
    try {
      const { output, stderr, exitCode } = await runProcess(
        "git",
        opts.args,
        this.workspaceCwd(),
      );
      const status = exitCode === 0 ? "ok" : "error";
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status,
          result: { exit: exitCode, output, stderr },
          duration_ms: Math.round(performance.now() - t0),
        },
        sensitivity: "low",
      });
      ledger.append({
        kind: "effect_observed",
        runId,
        payload: {
          kind: "git_exec",
          args: opts.args,
          exit: exitCode,
        },
        sensitivity: "low",
      });
      if (exitCode !== 0) {
        throw new BoxError(
          "git_failed",
          `git ${opts.args.join(" ")} failed (exit ${exitCode}): ${stderr.trim() || output.trim()}`,
        );
      }
      return { output };
    } catch (err) {
      if (err instanceof BoxError) throw err;
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status: "error",
          result: { error: (err as Error).message ?? String(err) },
          duration_ms: Math.round(performance.now() - t0),
        },
        sensitivity: "low",
      });
      throw new BoxError(
        "git_failed",
        `git ${opts.args.join(" ")} threw: ${(err as Error).message}`,
        err,
      );
    }
  }

  async checkout(opts: { branch: string; create?: boolean }): Promise<void> {
    const args = ["checkout"];
    if (opts.create) args.push("-b");
    args.push(opts.branch);
    await this.exec({ args });
  }

  // ----- internals -----

  /** Same as `exec`, but doesn't throw on nonzero — used by status helpers. */
  private async execSilent(opts: { args: string[] }): Promise<{ output: string; exitCode: number | null }> {
    const { output, exitCode } = await runProcess("git", opts.args, this.workspaceCwd());
    return { output, exitCode };
  }

  private workspaceCwd(): string {
    const { workspaceDir, cwd } = this.ctx();
    return cwd ? join(workspaceDir, cwd) : workspaceDir;
  }

  private recordEffect(kind: string, payload: Record<string, unknown>): void {
    const { ledger } = this.ctx();
    ledger.append({
      kind: "effect_observed",
      runId: `git-${ulid()}`,
      payload: { kind, ...payload },
      sensitivity: "low",
    });
  }

  private async hasOriginRemote(): Promise<boolean> {
    const { output, exitCode } = await runProcess(
      "git",
      ["remote", "get-url", "origin"],
      this.workspaceCwd(),
    ).catch(() => ({ output: "", stderr: "", exitCode: 1 }));
    return exitCode === 0 && output.trim().length > 0;
  }
}

// ----- helpers -----

interface RunResult {
  output: string;
  stderr: string;
  exitCode: number | null;
}

function runProcess(
  cmd: string,
  args: string[],
  cwd: string,
  env: NodeJS.ProcessEnv = process.env,
  opts: { timeoutMs?: number } = {},
): Promise<RunResult> {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, env });
    let output = "";
    let stderr = "";
    let settled = false;
    const timer =
      opts.timeoutMs === undefined
        ? undefined
        : setTimeout(() => {
            if (!settled) {
              settled = true;
              child.kill();
              reject(new Error(`${cmd} ${args.join(" ")} timed out after ${opts.timeoutMs}ms`));
            }
          }, opts.timeoutMs);
    const clearTimer = () => {
      if (timer !== undefined) clearTimeout(timer);
    };
    child.stdout?.on("data", (b: Buffer) => {
      output += b.toString("utf8");
    });
    child.stderr?.on("data", (b: Buffer) => {
      stderr += b.toString("utf8");
    });
    child.on("error", (err) => {
      if (!settled) {
        settled = true;
        clearTimer();
        reject(err);
      }
    });
    child.on("close", (code) => {
      if (!settled) {
        settled = true;
        clearTimer();
        resolve({ output, stderr, exitCode: code });
      }
    });
  });
}

async function hasBinary(bin: string): Promise<boolean> {
  return new Promise((res) => {
    const child = spawn(process.platform === "win32" ? "where" : "which", [bin]);
    let exited = false;
    child.on("error", () => {
      if (!exited) {
        exited = true;
        res(false);
      }
    });
    child.on("exit", (code) => {
      if (!exited) {
        exited = true;
        res(code === 0);
      }
    });
  });
}
