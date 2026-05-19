import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { ActantCallbackHandler } from "./index.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-langchain-test-"));
}

describe("@actantdb/langchain", () => {
  it("records model_call + tool events through the BaseCallbackHandler hooks", async () => {
    const dir = freshDir();
    try {
      const handler = new ActantCallbackHandler({
        project: "t",
        storeDir: dir,
      });

      // Simulate a LangChain run lifecycle.
      handler.handleChainStart({ name: "MyChain" }, { x: 1 }, "chain-1");
      handler.handleChatModelStart(
        { id: ["langchain", "chat", "ChatAnthropic"] },
        [[{ content: "Hello, world." }]],
        "llm-1",
        "chain-1",
        { modelName: "claude-sonnet" },
      );
      handler.handleLLMEnd(
        {
          generations: [
            [
              {
                message: {
                  usage_metadata: { input_tokens: 4, output_tokens: 9 },
                },
              },
            ],
          ],
        },
        "llm-1",
      );
      handler.handleToolStart(
        { id: ["langchain", "tools", "shell.run"] },
        JSON.stringify({ command: "ls" }),
        "tool-1",
        "chain-1",
      );
      handler.handleToolEnd({ exit: 0 }, "tool-1");
      handler.handleChainEnd({ output: "done" }, "chain-1");

      const events = handler.actant.ledger.query({});
      const kinds = events.map((e) => e.kind);
      expect(kinds).toContain("agent_run_started");
      expect(kinds).toContain("model_call");
      expect(kinds).toContain("tool_call_requested");
      expect(kinds).toContain("tool_call_started");
      expect(kinds).toContain("tool_call_completed");
      expect(kinds).toContain("agent_run_finished");

      const modelCall = events.find((e) => e.kind === "model_call")!;
      const mPayload = modelCall.payload as {
        model: string;
        tokens_in?: number;
        tokens_out?: number;
      };
      expect(mPayload.tokens_in).toBe(4);
      expect(mPayload.tokens_out).toBe(9);

      handler.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("records tool_call_completed with error status on tool error", async () => {
    const dir = freshDir();
    try {
      const handler = new ActantCallbackHandler({
        project: "t",
        storeDir: dir,
      });
      handler.handleChainStart({ name: "MyChain" }, { x: 1 }, "chain-1");
      handler.handleToolStart(
        { id: ["langchain", "tools", "bad"] },
        "raw-input",
        "tool-1",
        "chain-1",
      );
      handler.handleToolError(new Error("tool-boom"), "tool-1");
      handler.handleChainError(new Error("chain-boom"), "chain-1");
      const events = handler.actant.ledger.query({});
      const completed = events.find((e) => e.kind === "tool_call_completed")!;
      const payload = completed.payload as {
        status: string;
        result: { error?: string };
      };
      expect(payload.status).toBe("error");
      expect(payload.result.error).toContain("tool-boom");
      const finished = events.find((e) => e.kind === "agent_run_finished")!;
      const finishedPayload = finished.payload as {
        ok: boolean;
        error?: string;
      };
      expect(finishedPayload.ok).toBe(false);
      expect(finishedPayload.error).toContain("chain-boom");
      handler.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
