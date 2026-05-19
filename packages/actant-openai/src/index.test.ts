import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import OpenAI from "./index.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-openai-test-"));
}

class FakeUpstream {
  static behaviour: "ok" | "throw" = "ok";
  chat = {
    completions: {
      create: async (_params: unknown): Promise<unknown> => {
        if (FakeUpstream.behaviour === "throw") {
          throw new Error("openai-boom");
        }
        return {
          id: "chatcmpl_test",
          choices: [{ message: { role: "assistant", content: "hi" } }],
          usage: { prompt_tokens: 3, completion_tokens: 5 },
        };
      },
    },
  };
  responses = {
    create: async (_params: unknown): Promise<unknown> => ({
      id: "resp_test",
      output_text: "hi",
      usage: { input_tokens: 2, output_tokens: 4 },
    }),
  };
  constructor(public readonly opts: unknown) {}
}

describe("@actantdb/openai", () => {
  it("records a model_call for chat.completions.create", async () => {
    const dir = freshDir();
    try {
      const client = new OpenAI({
        apiKey: "sk-test",
        actant: { project: "t", storeDir: dir },
        _upstream: FakeUpstream as unknown as new (
          ...args: unknown[]
        ) => unknown,
      } as ConstructorParameters<typeof OpenAI>[0]);

      const result = (await (client as unknown as {
        chat: { completions: { create: (p: unknown) => Promise<unknown> } };
      }).chat.completions.create({
        model: "gpt-4o",
        messages: [{ role: "user", content: "Hello." }],
      })) as { id: string };

      expect(result.id).toBe("chatcmpl_test");
      const events = client.actant!.ledger.query({});
      const modelCall = events.find((e) => e.kind === "model_call");
      expect(modelCall).toBeDefined();
      const payload = modelCall!.payload as {
        model: string;
        tokens_in?: number;
        tokens_out?: number;
      };
      expect(payload.model).toBe("gpt-4o");
      expect(payload.tokens_in).toBe(3);
      expect(payload.tokens_out).toBe(5);
      client.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("records a model_call on chat error and rethrows", async () => {
    const dir = freshDir();
    FakeUpstream.behaviour = "throw";
    try {
      const client = new OpenAI({
        apiKey: "sk-test",
        actant: { project: "t", storeDir: dir },
        _upstream: FakeUpstream as unknown as new (
          ...args: unknown[]
        ) => unknown,
      } as ConstructorParameters<typeof OpenAI>[0]);

      await expect(
        (client as unknown as {
          chat: { completions: { create: (p: unknown) => Promise<unknown> } };
        }).chat.completions.create({
          model: "gpt-4o",
          messages: [{ role: "user", content: "Hi" }],
        }),
      ).rejects.toThrow("openai-boom");

      const events = client.actant!.ledger.query({});
      const modelCall = events.find((e) => e.kind === "model_call");
      expect(modelCall).toBeDefined();
      const payload = modelCall!.payload as { summary: string };
      expect(payload.summary).toContain("ERROR: openai-boom");
      client.close();
    } finally {
      FakeUpstream.behaviour = "ok";
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
