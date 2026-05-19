/**
 * Shared subprocess helpers for harness adapters.
 *
 * Every harness spawns a CLI on the host (or, eventually, inside the
 * cloud sandbox). Centralising the spawn + stream + buffer + timeout
 * logic keeps each adapter to ~40 LOC.
 */

import { spawn } from "node:child_process";
import type { ChildProcessWithoutNullStreams } from "node:child_process";

import { BoxError } from "../errors.js";
import type { AgentChunk } from "../types.js";

export interface SpawnInput {
  cli: string;
  args: string[];
  cwd: string;
  env: NodeJS.ProcessEnv;
  /** Sent to stdin and then closed. */
  stdin?: string;
  timeoutMs?: number;
}

export interface SpawnResult {
  exitCode: number | null;
  stdout: string;
  stderr: string;
  /** Wall-clock duration in ms. */
  computeMs: number;
}

/** Locate a CLI on PATH; honor `<UPPER>_PATH` env override if set. */
export function findCli(name: string): string {
  const envOverride = process.env[`${name.toUpperCase()}_PATH`];
  if (envOverride) return envOverride;
  return name;
}

/** Spawn + collect stdout/stderr; reject on timeout. */
export async function runOnce(input: SpawnInput): Promise<SpawnResult> {
  const t0 = performance.now();
  const child = spawn(input.cli, input.args, {
    cwd: input.cwd,
    env: { ...process.env, ...input.env },
    stdio: ["pipe", "pipe", "pipe"],
  }) as ChildProcessWithoutNullStreams;

  let stdout = "";
  let stderr = "";
  child.stdout.on("data", (d) => (stdout += String(d)));
  child.stderr.on("data", (d) => (stderr += String(d)));

  if (input.stdin !== undefined) {
    child.stdin.end(input.stdin);
  }

  const exit = new Promise<number | null>((resolve, reject) => {
    child.on("exit", (code) => resolve(code));
    child.on("error", (err) => {
      // Most common: ENOENT — CLI not installed.
      reject(
        new BoxError(
          "harness_cli_missing",
          `Could not spawn "${input.cli}" — install the CLI or set ${input.cli.toUpperCase()}_PATH. (${err.message})`,
        ),
      );
    });
  });

  let exitCode: number | null = null;
  if (input.timeoutMs && input.timeoutMs > 0) {
    const timeout = new Promise<never>((_, reject) =>
      setTimeout(
        () =>
          reject(new BoxError("harness_timeout", `${input.cli} exceeded ${input.timeoutMs} ms`)),
        input.timeoutMs,
      ),
    );
    try {
      exitCode = await Promise.race([exit, timeout]);
    } catch (e) {
      child.kill("SIGTERM");
      throw e;
    }
  } else {
    exitCode = await exit;
  }

  return {
    exitCode,
    stdout,
    stderr,
    computeMs: Math.round(performance.now() - t0),
  };
}

/**
 * Spawn + yield line-buffered stdout as `text-delta` chunks. Emits a
 * final `finish` chunk with the full result. Falls back gracefully if
 * the CLI emits JSON-per-line (harness adapters can parse those out
 * before forwarding).
 */
export async function* streamOnce(
  input: SpawnInput,
): AsyncGenerator<AgentChunk, void, void> {
  const child = spawn(input.cli, input.args, {
    cwd: input.cwd,
    env: { ...process.env, ...input.env },
    stdio: ["pipe", "pipe", "pipe"],
  }) as ChildProcessWithoutNullStreams;

  if (input.stdin !== undefined) {
    child.stdin.end(input.stdin);
  }

  let buf = "";
  let full = "";

  const lineQueue: string[] = [];
  let resolveNext: ((v: void) => void) | null = null;
  const wake = () => {
    const r = resolveNext;
    resolveNext = null;
    r?.();
  };

  child.stdout.on("data", (d) => {
    buf += String(d);
    const lines = buf.split(/\r?\n/);
    buf = lines.pop() ?? "";
    for (const ln of lines) {
      lineQueue.push(ln);
      full += ln + "\n";
    }
    wake();
  });

  let done = false;
  let exitCode: number | null = null;
  child.on("exit", (code) => {
    exitCode = code;
    if (buf) {
      lineQueue.push(buf);
      full += buf;
      buf = "";
    }
    done = true;
    wake();
  });

  let raised: Error | null = null;
  child.on("error", (err) => {
    raised = new BoxError(
      "harness_cli_missing",
      `Could not spawn "${input.cli}" — install the CLI or set ${input.cli.toUpperCase()}_PATH. (${err.message})`,
    );
    done = true;
    wake();
  });

  while (true) {
    if (raised) throw raised;
    while (lineQueue.length > 0) {
      const ln = lineQueue.shift()!;
      yield { type: "text-delta", text: ln + "\n" };
    }
    if (done) break;
    await new Promise<void>((r) => (resolveNext = r));
  }

  yield {
    type: "finish",
    result: { ok: exitCode === 0, exitCode, output: full },
  };
}
