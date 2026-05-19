import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { wrapAiSdk } from "./index.js";
import { demoPolicy } from "@actantdb/policy";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-ai-sdk-test-"));
}

describe("@actantdb/ai-sdk", () => {
  it("records model_call + tool_call events when generateText executes a tool", async () => {
    const dir = freshDir();
    try {
      const seen: Array<{ args: unknown }> = [];
      const fakeAi = {
        generateText: async (params: {
          tools?: Record<
            string,
            { execute?: (args: unknown) => Promise<unknown> }
          >;
        }) => {
          // Simulate the AI SDK invoking a tool from the model's planning.
          const echo = params.tools?.echo;
          let toolResult: unknown = undefined;
          if (echo?.execute) {
            toolResult = await echo.execute({ value: "ping" });
          }
          return { text: "done", toolResult };
        },
      };
      const wrapped = wrapAiSdk({
        project: "t",
        storeDir: dir,
        ai: fakeAi,
        autoApprove: true,
      });
      const result = (await wrapped.generateText({
        model: { provider: "openai", modelId: "gpt-4o" },
        messages: [{ role: "user", content: "hi" }],
        tools: {
          echo: {
            execute: async (args) => {
              seen.push({ args });
              return { ok: true };
            },
          },
        },
      })) as { text: string; toolResult: unknown };
      expect(result.text).toBe("done");
      expect(seen).toHaveLength(1);
      const events = wrapped.actant.ledger.query({});
      const kinds = events.map((e) => e.kind);
      expect(kinds).toContain("model_call");
      expect(kinds).toContain("tool_call_requested");
      expect(kinds).toContain("tool_call_completed");
      wrapped.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("records tool_call_completed with error status when the tool throws", async () => {
    const dir = freshDir();
    try {
      const fakeAi = {
        generateText: async (params: {
          tools?: Record<
            string,
            { execute?: (args: unknown) => Promise<unknown> }
          >;
        }) => {
          const fail = params.tools?.fail;
          // Wrap so the test can still inspect the ledger after the throw.
          if (fail?.execute) {
            try {
              await fail.execute({});
            } catch {
              // swallow — we want the upstream to "complete" so the test
              // can verify the ledger state.
            }
          }
          return { text: "swallowed" };
        },
      };
      const wrapped = wrapAiSdk({
        project: "t",
        storeDir: dir,
        ai: fakeAi,
        policy: demoPolicy,
        autoApprove: true,
      });
      await wrapped.generateText({
        model: "fake",
        messages: [],
        tools: {
          fail: {
            execute: async () => {
              throw new Error("kaboom");
            },
          },
        },
      });
      const events = wrapped.actant.ledger.query({});
      const completed = events.find((e) => e.kind === "tool_call_completed");
      expect(completed).toBeDefined();
      const payload = completed!.payload as {
        status: string;
        result: { error?: string };
      };
      expect(payload.status).toBe("error");
      expect(payload.result.error).toContain("kaboom");
      wrapped.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
