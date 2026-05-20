import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import {
  createActantElizaPlugin,
  withActantElizaAction,
  withActantElizaRuntime,
  type ElizaActionLike,
} from "./index.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-elizaos-test-"));
}

describe("@actantdb/elizaos", () => {
  it("records an action handler as a tool call", async () => {
    const dir = freshDir();
    try {
      const action = withActantElizaAction(
        {
          name: "SEND_REPLY",
          handler: async (_runtime: { agentId: string }, message: { content: { text: string } }) => {
            return { text: `sent: ${message.content.text}` };
          },
        },
        { project: "elizaos-test", storeDir: dir },
      );

      const result = await action.handler?.({ agentId: "agent_1" }, { content: { text: "hello" } });
      expect(result).toEqual({ text: "sent: hello" });
      const kinds = action.actant.ledger.query().map((event) => event.kind);
      expect(kinds).toContain("user_message_received");
      expect(kinds).toContain("tool_call_requested");
      expect(kinds).toContain("tool_call_completed");
      action.actant.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("wraps runtime actions without mutating the runtime object", async () => {
    const dir = freshDir();
    try {
      const action: ElizaActionLike<[{ text: string }], string> = {
        name: "ECHO",
        handler: (message) => message.text,
      };
      const runtime = { actions: [action], marker: "runtime" };
      const wrapped = withActantElizaRuntime(runtime, {
        project: "elizaos-runtime-test",
        storeDir: dir,
      });

      expect(wrapped).not.toBe(runtime);
      expect(runtime.actions[0]).toBe(action);
      await wrapped.actions?.[0]?.handler?.({ text: "hello" });
      expect(wrapped.actant.ledger.query({ kind: "tool_call_completed" })).toHaveLength(1);
      wrapped.actant.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("creates an elizaOS plugin shape with wrapped actions and a provider", async () => {
    const dir = freshDir();
    try {
      const plugin = createActantElizaPlugin({
        project: "elizaos-plugin-test",
        storeDir: dir,
        actions: [
          {
            name: "PING",
            handler: () => "pong",
          },
        ],
      });

      expect(plugin.name).toBe("actantdb");
      expect(await plugin.providers[0]?.get()).toEqual({
        text: "actantdb project=elizaos-plugin-test",
        values: expect.objectContaining({ project: "elizaos-plugin-test" }),
        data: expect.objectContaining({ project: "elizaos-plugin-test" }),
      });
      await plugin.actions[0]?.handler?.();
      expect(plugin.actant.ledger.query({ kind: "agent_run_finished" })).toHaveLength(1);
      plugin.actant.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
