import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { withActantTriggerTask } from "./index.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-triggerdev-test-"));
}

describe("@actantdb/triggerdev", () => {
  it("records a run around a Trigger.dev-shaped task", async () => {
    const dir = freshDir();
    try {
      const task = withActantTriggerTask(
        async (ctx: { task: { name: string }; payload: { id: string } }) => {
          return { ok: true, id: ctx.payload.id };
        },
        { project: "trigger-test", storeDir: dir },
      );
      await task({ task: { name: "sync-agent" }, payload: { id: "job_1" } });
      const events = task.actant.ledger.query();
      expect(events.map((event) => event.kind)).toContain("agent_run_started");
      expect(events.map((event) => event.kind)).toContain("effect_observed");
      task.actant.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
