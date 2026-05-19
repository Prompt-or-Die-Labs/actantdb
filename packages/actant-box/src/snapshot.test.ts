import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { Box } from "./index.js";

let root: string;

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "actantdb-box-snap-"));
});

afterEach(() => {
  rmSync(root, { recursive: true, force: true });
});

describe("box.snapshot", () => {
  it("snapshot + fromSnapshot restores workspace state", async () => {
    const box = await Box.create({ name: "src", storeRoot: root });
    await box.files.write({ path: "data/value.txt", content: "v1" });
    const snap = await box.snapshot({ name: "after-v1" });
    expect(snap.boxId).toBe(box.id);

    // Modify the original so we can prove the snapshot diverged.
    await box.files.write({ path: "data/value.txt", content: "v2" });
    expect(await box.files.read("data/value.txt")).toBe("v2");

    const restored = await Box.fromSnapshot(snap.id, { storeRoot: root, name: "restored" });
    expect(await restored.files.read("data/value.txt")).toBe("v1");

    // listSnapshots surfaces the snapshot (looked up against the original boxId).
    const snaps = await box.listSnapshots();
    expect(snaps.map((s) => s.id)).toContain(snap.id);

    // deleteSnapshot removes it.
    await box.deleteSnapshot(snap.id);
    const after = await box.listSnapshots();
    expect(after.map((s) => s.id)).not.toContain(snap.id);

    await box.delete();
    await restored.delete();
  });
});
