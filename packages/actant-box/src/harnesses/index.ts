/**
 * Harness registry. Pick by enum name (Agent.ClaudeCode etc) and the
 * Box agent layer dispatches transparently.
 */

import { claudeCode } from "./claude-code.js";
import { codex } from "./codex.js";
import { opencode } from "./opencode.js";
import { Agent, type AgentHarness, type Harness } from "./types.js";

export { Agent } from "./types.js";
export type { Harness, HarnessRunInput, HarnessRunResult, AgentHarness } from "./types.js";

const registry: Record<string, Harness> = {
  [Agent.ClaudeCode]: claudeCode,
  [Agent.Codex]: codex,
  [Agent.OpenCode]: opencode,
};

/** Lookup a harness adapter by enum value. Returns undefined for Custom. */
export function getHarness(name: AgentHarness | string): Harness | undefined {
  return registry[name];
}

/** All registered harnesses (display purposes). */
export function listHarnesses(): Harness[] {
  return Object.values(registry);
}
