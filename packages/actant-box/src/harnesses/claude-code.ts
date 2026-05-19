/**
 * Claude Code harness — spawns the `claude` CLI in the workspace.
 *
 * The CLI is one-shot: `claude --print "<prompt>"` runs the prompt
 * non-interactively in the cwd and writes its response to stdout. We
 * forward `--model` when supplied; we forward `ANTHROPIC_API_KEY` via env.
 *
 * Install: https://github.com/anthropics/claude-code or set CLAUDE_PATH
 * to point at the binary explicitly.
 */

import { runOnce, streamOnce, findCli } from "./spawn.js";
import type { Harness, HarnessRunInput, HarnessRunResult } from "./types.js";
import { ClaudeCode } from "../models.js";

class ClaudeCodeHarness implements Harness {
  readonly name = "claude-code";
  readonly defaultModel = ClaudeCode.Sonnet_4_6;
  readonly apiKeyEnv = "ANTHROPIC_API_KEY";
  get cliName(): string {
    return findCli("claude");
  }

  private buildArgs(input: HarnessRunInput): string[] {
    const args: string[] = ["--print", "--output-format", "text"];
    if (input.model && input.model.startsWith("anthropic/")) {
      args.push("--model", input.model.replace(/^anthropic\//, ""));
    } else if (input.model) {
      args.push("--model", input.model);
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
      stdin: input.prompt,
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
      stdin: input.prompt,
      ...(input.timeoutMs !== undefined ? { timeoutMs: input.timeoutMs } : {}),
    });
  }
}

export const claudeCode: Harness = new ClaudeCodeHarness();
