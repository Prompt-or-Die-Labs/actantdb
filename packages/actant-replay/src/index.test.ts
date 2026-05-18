import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { buildContextManifest, createActant } from "@actantdb/core";
import { demoPolicy } from "@actantdb/policy";

import { diff, diffReplayAgainstOriginal, runFromEvent, tighten } from "./index.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-replay-test-"));
}

describe("replay", () => {
  it("rebuilds the manifest minus excluded memory ids", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "r", storeDir: dir, policy: demoPolicy });
      const ctx = a.startRun();
      ctx.recordContextBuild(
        buildContextManifest([
          { id: "mem_42_dist", kind: "memory", source: "m", sensitivity: "low", label: "stale", content: "build artifacts under /build and /dist" },
          { id: "doc_readme", kind: "document", source: "f", sensitivity: "public", label: "readme", content: "readme text" },
        ]),
      );
      const planner = ctx.recordModelCall({ model: "x", role: "p", prompt_hash: "h", summary: "rm -rf build dist" });
      ctx.recordToolCallRequested({
        tool_call_id: "tc1",
        tool: "shell.run",
        args: { command: "rm -rf build dist" },
        risk: "destructive",
      });
      ctx.recordGuardVerdict("tc1", {
        decision: "require_approval",
        reason: "shell.run rm -rf with /dist",
        policy_snapshot: "snap",
      });
      const replay = runFromEvent({
        ledger: a.ledger,
        eventId: planner.id,
        overrides: { without_memory: ["mem_42_dist"] },
        policy: tighten(demoPolicy, {
          deny: [
            { tool: "shell.run", pattern: "\\bdist\\b", reason: "no dist" },
          ],
        }),
        alternatePlannerOutput: "rm -rf build",
      });
      const toolReq = replay.events.find((e) => e.kind === "tool_call_requested");
      expect(toolReq).toBeDefined();
      const args = (toolReq!.payload as { args: { command: string } }).args;
      expect(args.command).toBe("rm -rf build");
      const dif = diffReplayAgainstOriginal(a.ledger, replay);
      expect(dif.entries.length).toBeGreaterThan(0);
      expect(dif.entries.some((d) => d.diff === "changed")).toBe(true);
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("diff reports identical when streams match exactly", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "rr", storeDir: dir });
      const ctx = a.startRun();
      ctx.recordUserMessage("hi");
      const events = a.ledger.query({ runId: ctx.runId });
      const dif = diff(events, events);
      for (const e of dif.entries) expect(e.diff).toBe("identical");
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
