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

import {
  envForApiKey,
  runHarnessCommand,
  streamHarnessCommand,
  type HarnessCommandInput,
} from "./command.js";
import { findCli } from "./spawn.js";
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

  private command(input: HarnessRunInput): HarnessCommandInput {
    return {
      cli: this.cliName,
      args: this.buildArgs(input),
      cwd: input.cwd,
      env: envForApiKey(this.apiKeyEnv, input),
      ...(input.timeoutMs !== undefined ? { timeoutMs: input.timeoutMs } : {}),
    };
  }

  async run(input: HarnessRunInput): Promise<HarnessRunResult> {
    return runHarnessCommand(this.command(input));
  }

  stream(input: HarnessRunInput) {
    return streamHarnessCommand(this.command(input));
  }
}

export const codex: Harness = new CodexHarness();
