/**
 * OpenCode CLI harness — spawns the `opencode` CLI in the workspace.
 *
 * Install: https://opencode.ai (npm or curl installer). Override the
 * binary location with `OPENCODE_PATH`.
 *
 * OpenCode is interactive by default; we use `opencode run "<prompt>"`
 * for a non-interactive one-shot. `--model` selects the model id.
 *
 * API key env vars depend on the model provider OpenCode is routed at —
 * we forward whatever the consumer passes via `apiKey` to the most
 * likely env var (`OPENROUTER_API_KEY` since OpenCode defaults to
 * OpenRouter); consumers configuring a different provider should set
 * the env var themselves before `Box.create`.
 */

import {
  envForApiKey,
  runHarnessCommand,
  streamHarnessCommand,
  type HarnessCommandInput,
} from "./command.js";
import { findCli } from "./spawn.js";
import type { Harness, HarnessRunInput, HarnessRunResult } from "./types.js";
import { OpenCodeModel } from "../models.js";

class OpenCodeHarness implements Harness {
  readonly name = "opencode";
  readonly defaultModel = OpenCodeModel.Claude_Sonnet_4_6;
  readonly apiKeyEnv = "OPENROUTER_API_KEY";
  get cliName(): string {
    return findCli("opencode");
  }

  private buildArgs(input: HarnessRunInput): string[] {
    const args: string[] = ["run", input.prompt];
    if (input.model) {
      args.push("--model", input.model);
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

export const opencode: Harness = new OpenCodeHarness();
