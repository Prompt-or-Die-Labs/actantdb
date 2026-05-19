/**
 * OpenAI Codex CLI harness — spawns the `codex` CLI in the workspace.
 *
 * Install: `npm install -g @openai/codex` or set CODEX_PATH.
 *
 * Codex accepts a prompt as a positional argument under `exec`:
 *
 *     codex exec "<prompt>"
 *
 * `--model` selects the variant; `OPENAI_API_KEY` carries auth.
 */

import { runOnce, streamOnce, findCli } from "./spawn.js";
import type { Harness, HarnessRunInput, HarnessRunResult } from "./types.js";
import { OpenAICodex } from "../models.js";

class CodexHarness implements Harness {
  readonly name = "codex";
  readonly defaultModel = OpenAICodex.GPT_5_4;
  readonly apiKeyEnv = "OPENAI_API_KEY";
  get cliName(): string {
    return findCli("codex");
  }

  private buildArgs(input: HarnessRunInput): string[] {
    const args: string[] = ["exec", input.prompt];
    if (input.model) {
      // Strip the `openai/` scheme prefix; codex expects the bare model id.
      const m = input.model.replace(/^openai\//, "");
      args.push("--model", m);
    }
    if (input.extraArgs) args.push(...input.extraArgs);
    return args;
  }

  private buildEnv(input: HarnessRunInput): NodeJS.ProcessEnv {
    const env: NodeJS.ProcessEnv = {};
    if (input.apiKey) env[this.apiKeyEnv] = input.apiKey;
    return env;
  }

  async run(input: HarnessRunInput): Promise<HarnessRunResult> {
    const r = await runOnce({
      cli: this.cliName,
      args: this.buildArgs(input),
      cwd: input.cwd,
      env: this.buildEnv(input),
      ...(input.timeoutMs !== undefined ? { timeoutMs: input.timeoutMs } : {}),
    });
    return {
      ok: r.exitCode === 0,
      output: r.stdout + (r.stderr ? `\n--- stderr ---\n${r.stderr}` : ""),
      result: r.stdout.trim(),
      computeMs: r.computeMs,
      exitCode: r.exitCode,
    };
  }

  stream(input: HarnessRunInput) {
    return streamOnce({
      cli: this.cliName,
      args: this.buildArgs(input),
      cwd: input.cwd,
      env: this.buildEnv(input),
      ...(input.timeoutMs !== undefined ? { timeoutMs: input.timeoutMs } : {}),
    });
  }
}

export const codex: Harness = new CodexHarness();
