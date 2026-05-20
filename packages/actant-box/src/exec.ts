/**
 * @actantdb/box — exec namespace.
 *
 * Spawns subprocesses inside the workspace dir (or workspace/cwd). All output
 * is captured and persisted via tool_call_completed; `box.exec.stream`
 * additionally yields line chunks for live consumption.
 *
 * Tool name: "exec.command".
 */

import { spawn, type ChildProcess } from "node:child_process";
import { join } from "node:path";

import { ulid, type Ledger } from "@actantdb/core";

import { BoxError } from "./errors.js";
import { Run } from "./run.js";
import type { ExecChunk } from "./types.js";

export interface ExecCtx {
  ledger: Ledger;
  workspaceDir: string;
  cwd: string;
}
type CtxProvider = () => ExecCtx;

export interface ExecOptions {
  /** Override cwd for this call (joined with workspace if relative). */
  cwd?: string;
  /** Env vars merged into process.env. */
  env?: Record<string, string>;
  /** Timeout in ms before SIGTERM. */
  timeoutMs?: number;
  /**
   * Override the default `/bin/sh -c <cmd>` (POSIX) invocation. When
   * `args` is provided, `cmd` is treated as an executable name.
   */
  args?: string[];
}

export class BoxExecAPI {
  constructor(private readonly ctx: CtxProvider) {}

  /** Run a shell command. Output is captured + persisted; returns a Run. */
  async command(cmd: string, opts: ExecOptions = {}): Promise<Run> {
    const { ledger, workspaceDir, cwd } = this.ctx();
    const runId = `exec-${ulid()}`;
    const toolCallId = ulid();
    const t0 = performance.now();

    ledger.append({
      kind: "tool_call_requested",
      runId,
      payload: {
        tool_call_id: toolCallId,
        tool: "exec.command",
        args: { command: cmd, cwd: opts.cwd ?? cwd },
        risk: "medium",
      },
      sensitivity: "low",
    });
    ledger.append({
      kind: "tool_call_started",
      runId,
      payload: { tool_call_id: toolCallId, final_args: { command: cmd } },
      sensitivity: "low",
    });

    let child: ChildProcess | undefined;
    const run = new Run({
      id: runId,
      ledger,
      cancel: () => {
        try {
          child?.kill("SIGTERM");
        } catch {
          /* ignore */
        }
      },
    });
    run.markRunning();

    try {
      const { stdout, stderr, exitCode } = await spawnAndCapture(
        cmd,
        opts,
        resolveCwd(workspaceDir, cwd, opts.cwd),
        (c) => {
          child = c;
        },
      );
      const ms = Math.round(performance.now() - t0);
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status: exitCode === 0 ? "ok" : "error",
          result: { exit: exitCode, output: stdout, stderr },
          duration_ms: ms,
        },
        sensitivity: "low",
      });
      ledger.append({
        kind: "effect_observed",
        runId,
        payload: {
          kind: "exec_completed",
          command: cmd,
          exit: exitCode,
          stdout_bytes: Buffer.byteLength(stdout, "utf8"),
        },
        sensitivity: "low",
      });
      run.complete({ exit: exitCode, output: stdout, stderr });
      if (exitCode !== 0) {
        // Mirror Upstash: a failing process resolves to an `error` Run rather
        // than throwing — consumers inspect `run.status`. But surface a real
        // BoxError when there was no exit code at all.
        run.status = "error";
      }
      return run;
    } catch (err) {
      const ms = Math.round(performance.now() - t0);
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status: "error",
          result: { error: (err as Error).message ?? String(err) },
          duration_ms: ms,
        },
        sensitivity: "low",
      });
      run.fail(err);
      throw new BoxError(
        "exec_failed",
        `exec.command failed: ${(err as Error).message}`,
        err,
      );
    }
  }

  /**
   * Run a shell command, yielding line-buffered stdout/stderr chunks then a
   * final exit chunk. The associated ledger entries are written when the
   * stream ends.
   */
  async *stream(cmd: string, opts: ExecOptions = {}): AsyncIterable<ExecChunk> {
    const { ledger, workspaceDir, cwd } = this.ctx();
    const runId = `exec-${ulid()}`;
    const toolCallId = ulid();
    const t0 = performance.now();

    ledger.append({
      kind: "tool_call_requested",
      runId,
      payload: {
        tool_call_id: toolCallId,
        tool: "exec.command",
        args: { command: cmd, cwd: opts.cwd ?? cwd, streaming: true },
        risk: "medium",
      },
      sensitivity: "low",
    });
    ledger.append({
      kind: "tool_call_started",
      runId,
      payload: { tool_call_id: toolCallId, final_args: { command: cmd } },
      sensitivity: "low",
    });

    const cwdAbs = resolveCwd(workspaceDir, cwd, opts.cwd);
    const child = spawnRaw(cmd, opts, cwdAbs);

    const queue: ExecChunk[] = [];
    let waiter: ((v: void) => void) | null = null;
    const wake = () => {
      const w = waiter;
      waiter = null;
      if (w) w();
    };
    const onStdoutLine = (line: string) => {
      queue.push({ type: "stdout", line });
      wake();
    };
    const onStderrLine = (line: string) => {
      queue.push({ type: "stderr", line });
      wake();
    };
    let stdoutAll = "";
    let stderrAll = "";
    bufferLines(child.stdout, (l) => {
      stdoutAll += l + "\n";
      onStdoutLine(l);
    });
    bufferLines(child.stderr, (l) => {
      stderrAll += l + "\n";
      onStderrLine(l);
    });

    let exitCode: number | null = null;
    let done = false;
    child.on("close", (code) => {
      exitCode = code;
      done = true;
      queue.push({ type: "exit", code });
      wake();
    });
    child.on("error", (err) => {
      queue.push({ type: "stderr", line: `[spawn error] ${err.message}` });
      done = true;
      wake();
    });

    try {
      while (true) {
        if (queue.length) {
          const c = queue.shift()!;
          yield c;
          if (c.type === "exit") break;
          continue;
        }
        if (done) break;
        await new Promise<void>((res) => {
          waiter = res;
        });
      }
    } finally {
      const ms = Math.round(performance.now() - t0);
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status: exitCode === 0 ? "ok" : "error",
          result: { exit: exitCode, output: stdoutAll, stderr: stderrAll },
          duration_ms: ms,
        },
        sensitivity: "low",
      });
      ledger.append({
        kind: "effect_observed",
        runId,
        payload: {
          kind: "exec_completed",
          command: cmd,
          exit: exitCode,
          stdout_bytes: Buffer.byteLength(stdoutAll, "utf8"),
        },
        sensitivity: "low",
      });
    }
  }
}

// ----- helpers -----

function resolveCwd(workspaceDir: string, cwd: string, override?: string): string {
  if (override) {
    return override.startsWith("/") ? override : join(workspaceDir, cwd, override);
  }
  return cwd ? join(workspaceDir, cwd) : workspaceDir;
}

interface CaptureResult {
  stdout: string;
  stderr: string;
  exitCode: number | null;
}

function spawnRaw(cmd: string, opts: ExecOptions, cwd: string): ChildProcess {
  const env = opts.env === undefined ? process.env : { ...process.env, ...opts.env };
  if (opts.args && opts.args.length) {
    return spawn(cmd, opts.args, { cwd, env });
  }
  return spawn(cmd, { cwd, env, shell: true });
}

function spawnAndCapture(
  cmd: string,
  opts: ExecOptions,
  cwd: string,
  onSpawn: (c: ChildProcess) => void,
): Promise<CaptureResult> {
  return new Promise((resolve, reject) => {
    const child = spawnRaw(cmd, opts, cwd);
    onSpawn(child);
    let stdout = "";
    let stderr = "";
    child.stdout?.on("data", (b: Buffer) => {
      stdout += b.toString("utf8");
    });
    child.stderr?.on("data", (b: Buffer) => {
      stderr += b.toString("utf8");
    });

    let timer: NodeJS.Timeout | undefined;
    if (opts.timeoutMs && opts.timeoutMs > 0) {
      timer = setTimeout(() => {
        try {
          child.kill("SIGTERM");
        } catch {
          /* ignore */
        }
      }, opts.timeoutMs);
    }

    child.on("error", (err) => {
      if (timer) clearTimeout(timer);
      reject(err);
    });
    child.on("close", (code) => {
      if (timer) clearTimeout(timer);
      resolve({ stdout, stderr, exitCode: code });
    });
  });
}

function bufferLines(
  stream: NodeJS.ReadableStream | null | undefined,
  onLine: (line: string) => void,
): void {
  if (!stream) return;
  let buf = "";
  stream.on("data", (chunk: Buffer | string) => {
    buf += typeof chunk === "string" ? chunk : chunk.toString("utf8");
    let idx: number;
    while ((idx = buf.indexOf("\n")) !== -1) {
      const line = buf.slice(0, idx);
      buf = buf.slice(idx + 1);
      onLine(line);
    }
  });
  stream.on("end", () => {
    if (buf.length) {
      onLine(buf);
      buf = "";
    }
  });
}
