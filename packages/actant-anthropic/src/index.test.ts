import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import Anthropic from "./index.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-anthropic-test-"));
}

class FakeUpstream {
  static lastArgs: unknown[] | undefined;
  static behaviour: "ok" | "throw" = "ok";
  messages = {
    create: async (...args: unknown[]): Promise<unknown> => {
      FakeUpstream.lastArgs = args;
      if (FakeUpstream.behaviour === "throw") {
        throw new Error("upstream-boom");
      }
      return {
        id: "msg_test",
        content: [{ type: "text", text: "hello" }],
        usage: { input_tokens: 7, output_tokens: 11 },
      };
    },
  };
  someOtherProp = "passthrough-value";
  someOtherMethod(): string {
    return "passthrough-method";
  }
  constructor(public readonly opts: unknown) {}
}

describe("@actantdb/anthropic", () => {
  it("records a model_call on a successful messages.create", async () => {
    const dir = freshDir();
    try {
      const client = new Anthropic({
        apiKey: "sk-test",
        actant: { project: "t", storeDir: dir },
        _upstream: FakeUpstream as unknown as new (
          ...args: unknown[]
        ) => unknown,
      } as ConstructorParameters<typeof Anthropic>[0]);

      const result = (await (client as unknown as {
        messages: { create: (p: unknown) => Promise<unknown> };
      }).messages.create({
        model: "claude-sonnet",
        messages: [{ role: "user", content: "Hi" }],
      })) as { id: string };

      expect(result.id).toBe("msg_test");
      const events = client.actant!.ledger.query({});
      const kinds = events.map((e) => e.kind);
      expect(kinds).toContain("model_call");
      const modelCall = events.find((e) => e.kind === "model_call")!;
      const payload = modelCall.payload as {
        model: string;
        tokens_in?: number;
        tokens_out?: number;
      };
      expect(payload.model).toBe("claude-sonnet");
      expect(payload.tokens_in).toBe(7);
      expect(payload.tokens_out).toBe(11);
      client.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("records a model_call when the upstream throws and rethrows the error", async () => {
    const dir = freshDir();
    FakeUpstream.behaviour = "throw";
    try {
      const client = new Anthropic({
        apiKey: "sk-test",
        actant: { project: "t", storeDir: dir },
        _upstream: FakeUpstream as unknown as new (
          ...args: unknown[]
        ) => unknown,
      } as ConstructorParameters<typeof Anthropic>[0]);

      await expect(
        (client as unknown as {
          messages: { create: (p: unknown) => Promise<unknown> };
        }).messages.create({
          model: "claude-sonnet",
          messages: [{ role: "user", content: "Hi" }],
        }),
      ).rejects.toThrow("upstream-boom");

      const events = client.actant!.ledger.query({});
      const modelCall = events.find((e) => e.kind === "model_call");
      expect(modelCall).toBeDefined();
      const payload = modelCall!.payload as { summary: string };
      expect(payload.summary).toContain("ERROR: upstream-boom");
      client.close();
    } finally {
      FakeUpstream.behaviour = "ok";
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
