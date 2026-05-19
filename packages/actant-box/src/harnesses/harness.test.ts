import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { Box } from "../box.js";
import { Agent, ClaudeCode, OpenAICodex, OpenCodeModel, getHarness, listHarnesses } from "../index.js";

describe("harnesses", () => {
  let root: string;
  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), "actantdb-box-harness-"));
  });
  afterEach(() => {
    rmSync(root, { recursive: true, force: true });
  });

  it("Agent enum exposes the four Upstash-Box-parity presets", () => {
    expect(Agent.ClaudeCode).toBe("claude-code");
    expect(Agent.Codex).toBe("codex");
    expect(Agent.OpenCode).toBe("opencode");
    expect(Agent.Cursor).toBe("cursor");
  });

  it("Model preset enums carry canonical <provider>/<model> ids", () => {
    expect(ClaudeCode.Sonnet_4_6).toBe("anthropic/claude-sonnet-4-6");
    expect(ClaudeCode.Opus_4_7).toBe("anthropic/claude-opus-4-7");
    expect(OpenAICodex.GPT_5_4).toBe("openai/gpt-5.4");
    expect(OpenCodeModel.Claude_Sonnet_4_6).toBe("anthropic/claude-sonnet-4-6");
  });

  it("getHarness returns adapters for known presets, undefined otherwise", () => {
    expect(getHarness(Agent.ClaudeCode)?.name).toBe("claude-code");
    expect(getHarness(Agent.Codex)?.name).toBe("codex");
    expect(getHarness(Agent.OpenCode)?.name).toBe("opencode");
    expect(getHarness("not-a-harness")).toBeUndefined();
  });

  it("listHarnesses surfaces every registered preset", () => {
    const names = listHarnesses().map((h) => h.name).sort();
    expect(names).toEqual(["claude-code", "codex", "opencode"]);
  });

  it("Box.create wires a preset harness without throwing", async () => {
    const box = await Box.create({
      id: "harness-config",
      storeRoot: root,
      agent: {
        harness: Agent.ClaudeCode,
        model: ClaudeCode.Sonnet_4_6,
        apiKey: "stub-key",
      },
    });
    expect(box.modelConfig.harness).toBe("claude-code");
    expect(box.modelConfig.model).toBe("anthropic/claude-sonnet-4-6");
    await box.delete();
  });

  it("Box.create still accepts a bare custom agent (backwards compat)", async () => {
    const box = await Box.create({
      id: "custom-agent",
      storeRoot: root,
      agent: { tools: {} },
    });
    expect(box.modelConfig.harness).toBeUndefined();
    await box.delete();
  });

  it("unknown harness throws a typed BoxError", async () => {
    await expect(
      Box.create({
        id: "bad-harness",
        storeRoot: root,
        agent: { harness: "made-up-harness" as never },
      }),
    ).rejects.toThrow(/unknown harness/i);
  });
});
