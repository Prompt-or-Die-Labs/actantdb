import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { Box } from "./index.js";

let root: string;

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "actantdb-box-sched-"));
});

afterEach(() => {
  rmSync(root, { recursive: true, force: true });
});

const wait = (ms: number) => new Promise((res) => setTimeout(res, ms));

describe("box.schedule", () => {
  it("exec schedule fires at least once and records a schedule_fired effect", async () => {
    const box = await Box.create({ storeRoot: root });
    const s = await box.schedule.exec({ everyMs: 60, command: "echo tick" });
    try {
      await wait(220);
      const list = await box.schedule.list();
      expect(list.length).toBe(1);
      expect(list[0]!.runs).toBeGreaterThanOrEqual(1);
      const effects = box.ledger
        .query({})
        .map((e) => e.payload as { kind?: string })
        .filter((p) => p.kind === "schedule_fired");
      expect(effects.length).toBeGreaterThan(0);
    } finally {
      await box.schedule.delete(s.id);
      await box.delete();
    }
  });

  it("pause + resume + delete behave as labelled", async () => {
    const box = await Box.create({ storeRoot: root });
    const s = await box.schedule.exec({ everyMs: 50, command: "echo x" });
    try {
      await wait(80);
      await box.schedule.pause(s.id);
      const paused = await box.schedule.get(s.id);
      expect(paused.status).toBe("paused");
      const runsAtPause = paused.runs;
      await wait(120);
      const stillPaused = await box.schedule.get(s.id);
      expect(stillPaused.runs).toBe(runsAtPause);

      await box.schedule.resume(s.id);
      await wait(120);
      const resumed = await box.schedule.get(s.id);
      expect(resumed.runs).toBeGreaterThan(runsAtPause);
    } finally {
      await box.schedule.delete(s.id);
      await box.delete();
    }
  });
});
