import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { withActantAgent } from "./index.js";
import { demoPolicy } from "@actantdb/policy";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-openai-agents-test-"));
}

describe("@actantdb/openai-agents", () => {
  it("captures model_call + tool events for a happy-path run", async () => {
    const dir = freshDir();
    try {
      const calls: Array<{ args: unknown }> = [];
      const agent = {
        name: "support",
        model: "gpt-4o",
        tools: [
          {
            name: "echo",
            invoke: async (args: unknown) => {
              calls.push({ args });
              return { ok: true };
            },
          },
        ],
        run: async (_input: unknown) => {
          // Simulate the agent invoking its own tool.
          await agent.tools[0]!.invoke!({ value: "hi" });
          return { output: "done" };
        },
      };
      const wrapped = withActantAgent(agent, {
        project: "t",
        storeDir: dir,
        autoApprove: true,
      });
      const { result } = await wrapped.run({ message: "hello" });
      expect(result).toEqual({ output: "done" });
      expect(calls).toHaveLength(1);
      const events = wrapped.actant.ledger.query({});
      const kinds = events.map((e) => e.kind);
      expect(kinds).toContain("agent_run_started");
      expect(kinds).toContain("model_call");
      expect(kinds).toContain("tool_call_requested");
      expect(kinds).toContain("tool_call_completed");
      expect(kinds).toContain("agent_run_finished");
      wrapped.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("records agent_run_finished with error and rethrows when agent.run throws", async () => {
    const dir = freshDir();
    try {
      const agent = {
        name: "broken",
        model: "gpt-4o",
        tools: [],
        run: async () => {
          throw new Error("agent-boom");
        },
      };
      const wrapped = withActantAgent(agent, {
        project: "t",
        storeDir: dir,
        policy: demoPolicy,
      });
      await expect(wrapped.run({ message: "hi" })).rejects.toThrow(
        "agent-boom",
      );
      const events = wrapped.actant.ledger.query({});
      const finished = events.find((e) => e.kind === "agent_run_finished");
      expect(finished).toBeDefined();
      const payload = finished!.payload as { ok: boolean; error?: string };
      expect(payload.ok).toBe(false);
      expect(payload.error).toContain("agent-boom");
      wrapped.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
