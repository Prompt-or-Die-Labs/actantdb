import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { createActant } from "@actantdb/core";

import { startStudioServer } from "./server.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-studio-test-"));
}

describe("Studio HTTP API", () => {
  it("exposes /api/info, /api/events, /api/replay", async () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "s", storeDir: dir });
      const ctx = a.startRun();
      ctx.recordUserMessage("hi");
      const planner = ctx.recordModelCall({
        model: "x",
        role: "p",
        prompt_hash: "h",
        summary: "s",
      });
      const studio = await startStudioServer({ ledger: a.ledger, port: 0, silent: true });
      try {
        const info = await (await fetch(`${studio.url}/api/info`)).json();
        expect(info.project).toBe("s");
        expect(Array.isArray(info.runs)).toBe(true);
        const events = await (
          await fetch(`${studio.url}/api/events?run=${encodeURIComponent(ctx.runId)}`)
        ).json();
        expect(events.events.length).toBeGreaterThan(0);
        const replay = await (
          await fetch(`${studio.url}/api/replay`, {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ eventId: planner.id, overrides: { without_memory: [] } }),
          })
        ).json();
        expect(replay.diff).toBeDefined();
      } finally {
        await studio.close();
      }
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
