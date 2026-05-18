import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { withActant } from "./index.js";
import { demoPolicy } from "@actantdb/policy";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-mastra-test-"));
}

describe("withActant", () => {
  it("captures the killer-demo flow and constrains the destructive command", async () => {
    const dir = freshDir();
    try {
      const seen: Array<{ tool: string; args: unknown }> = [];
      const agent = {
        tools: {
          "shell.run": {
            execute: async (args: unknown) => {
              // Stock tool shape: receives the args Guard sealed.
              seen.push({ tool: "shell.run", args });
              return { exit: 0 };
            },
          },
        },
        generate: async () => "done",
      };
      const wrapped = withActant(agent, {
        project: "t",
        storeDir: dir,
        policy: demoPolicy,
        autoApprove: true,
      });
      const ctx = wrapped.startRun();
      ctx.recordUserMessage("clean");
      await agent.tools["shell.run"]!.execute({ command: "rm -rf build dist" });
      ctx.finish();
      const events = wrapped.actant.ledger.query({ runId: ctx.runId });
      const kinds = events.map((e) => e.kind);
      expect(kinds).toContain("guard_verdict");
      expect(kinds).toContain("approval_required");
      expect(kinds).toContain("approval_decision");
      expect(kinds).toContain("tool_call_completed");
      expect(seen[0]!.args).toEqual({ command: "rm -rf build" });
      wrapped.actant.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("denies when no approver and no autoApprove", async () => {
    const dir = freshDir();
    try {
      const agent = {
        tools: {
          "shell.run": {
            execute: async () => ({ exit: 0 }),
          },
        },
        generate: async () => "done",
      };
      const wrapped = withActant(agent, {
        project: "t",
        storeDir: dir,
        policy: demoPolicy,
      });
      const ctx = wrapped.startRun();
      const result = await agent.tools["shell.run"]!.execute({ command: "ls" });
      ctx.finish();
      expect((result as { denied?: boolean }).denied).toBe(true);
      wrapped.actant.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
