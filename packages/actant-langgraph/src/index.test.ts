import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { withActant, withLangGraph } from "./index.js";
import { demoPolicy } from "@actantdb/policy";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-langgraph-test-"));
}

describe("@actantdb/langgraph", () => {
  it("exports the existing withActant wrapper by LangGraph package name", async () => {
    const dir = freshDir();
    try {
      const seen: Array<{ tool: string; args: unknown }> = [];
      const graph = {
        name: "router-graph",
        tools: {
          "shell.run": {
            execute: async (args: unknown) => {
              seen.push({ tool: "shell.run", args });
              return { exit: 0 };
            },
          },
        },
      };
      const wrapped = withActant(graph, {
        project: "t",
        storeDir: dir,
        policy: demoPolicy,
        autoApprove: true,
      });
      const ctx = wrapped.startRun({ meta: { source: "@actantdb/langgraph" } });
      await graph.tools["shell.run"]!.execute({ command: "rm -rf cache dist" });
      ctx.finish();
      const events = wrapped.actant.ledger.query({ runId: ctx.runId });
      expect(events.map((event) => event.kind)).toContain("guard_verdict");
      expect(events.map((event) => event.kind)).toContain("approval_decision");
      expect(seen[0]!.args).toEqual({ command: "rm -rf cache" });
      expect(withLangGraph).toBe(withActant);
      wrapped.actant.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
