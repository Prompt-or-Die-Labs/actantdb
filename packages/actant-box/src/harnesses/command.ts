import { runOnce, streamOnce, type SpawnInput } from "./spawn.js";
import type { HarnessRunInput, HarnessRunResult } from "./types.js";

export interface HarnessCommandInput {
  cli: string;
  args: string[];
  cwd: string;
  env: NodeJS.ProcessEnv;
  stdin?: string;
  timeoutMs?: number;
}

export function envForApiKey(apiKeyEnv: string, input: HarnessRunInput): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = {};
  if (input.apiKey) env[apiKeyEnv] = input.apiKey;
  return env;
}

export async function runHarnessCommand(
  command: HarnessCommandInput,
): Promise<HarnessRunResult> {
  const r = await runOnce(spawnInput(command));
  return {
    ok: r.exitCode === 0,
    output: r.stdout + (r.stderr ? `\n--- stderr ---\n${r.stderr}` : ""),
    result: r.stdout.trim(),
    computeMs: r.computeMs,
    exitCode: r.exitCode,
  };
}

export function streamHarnessCommand(command: HarnessCommandInput) {
  return streamOnce(spawnInput(command));
}

function spawnInput(command: HarnessCommandInput): SpawnInput {
  return {
    cli: command.cli,
    args: command.args,
    cwd: command.cwd,
    env: command.env,
    ...(command.stdin !== undefined ? { stdin: command.stdin } : {}),
    ...(command.timeoutMs !== undefined ? { timeoutMs: command.timeoutMs } : {}),
  };
}
