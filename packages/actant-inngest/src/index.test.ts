import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { withActantInngest } from "./index.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-inngest-test-"));
}

describe("@actantdb/inngest", () => {
  it("records a run around an Inngest-shaped handler", async () => {
    const dir = freshDir();
    try {
      const handler = withActantInngest(
        async (ctx: { event: { name: string; data: { id: string } } }) => {
          return { ok: true, id: ctx.event.data.id };
        },
        { project: "inngest-test", storeDir: dir },
      );
      await handler({ event: { name: "agent/run", data: { id: "evt_1" } } });
      const events = handler.actant.ledger.query();
      expect(events.map((event) => event.kind)).toContain("agent_run_started");
      expect(events.map((event) => event.kind)).toContain("effect_observed");
      handler.actant.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
