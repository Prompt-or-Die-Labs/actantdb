import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { Box } from "./index.js";

let root: string;

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "actantdb-box-"));
});

afterEach(() => {
  rmSync(root, { recursive: true, force: true });
});

describe("Box lifecycle", () => {
  it("creates a box with a workspace dir, ledger, and box.json", async () => {
    const box = await Box.create({ name: "demo", storeRoot: root });
    expect(box.id).toBeTruthy();
    expect(box.name).toBe("demo");
    expect(existsSync(box.workspaceDir)).toBe(true);
    expect(existsSync(join(root, box.id, "box.json"))).toBe(true);
    // ledger has at least the box_created effect
    const events = box.ledger.query({});
    expect(events.length).toBeGreaterThan(0);
    expect(events[0]!.kind).toBe("effect_observed");
    await box.delete();
  });

  it("get reopens a previously created box by id", async () => {
    const created = await Box.create({ name: "persist", storeRoot: root });
    const id = created.id;
    // close ledger so re-open doesn't crash on shared file (best-effort)
    created.ledger.close();

    const reopened = await Box.get(id, { storeRoot: root });
    expect(reopened.id).toBe(id);
    expect(reopened.name).toBe("persist");
    await reopened.delete();
  });

  it("getByName resolves the most recent box with a given name", async () => {
    const a = await Box.create({ name: "alpha", storeRoot: root });
    a.ledger.close();
    const reopened = await Box.getByName("alpha", { storeRoot: root });
    expect(reopened.id).toBe(a.id);
    await reopened.delete();
  });

  it("list returns metadata for all known boxes", async () => {
    const a = await Box.create({ name: "a", storeRoot: root });
    const b = await Box.create({ name: "b", storeRoot: root });
    const boxes = await Box.list({ storeRoot: root });
    expect(boxes.map((x) => x.name).sort()).toEqual(["a", "b"]);
    await a.delete();
    await b.delete();
  });

  it("cd updates cwd and refuses to escape the workspace", async () => {
    const box = await Box.create({ storeRoot: root });
    await box.files.write({ path: "sub/file.txt", content: "hi" });
    await box.cd("sub");
    expect(box.cwd).toBe("sub");
    await expect(box.cd("../../../etc")).rejects.toThrow();
    await box.delete();
  });

  it("pause/resume flips status and records ledger events", async () => {
    const box = await Box.create({ storeRoot: root });
    await box.pause();
    expect((await box.getStatus()).status).toBe("paused");
    await box.resume();
    expect((await box.getStatus()).status).toBe("running");
    const kinds = box.ledger.query({}).map((e) => e.payload as { kind?: string });
    expect(kinds.some((k) => k.kind === "box_paused")).toBe(true);
    expect(kinds.some((k) => k.kind === "box_resumed")).toBe(true);
    await box.delete();
  });

  it("delete removes the workspace dir", async () => {
    const box = await Box.create({ storeRoot: root });
    const boxRoot = join(root, box.id);
    expect(existsSync(boxRoot)).toBe(true);
    await box.delete();
    expect(existsSync(boxRoot)).toBe(false);
  });

  it("cloud mode throws on instance ops but Box.create resolves", async () => {
    const box = await Box.create({ storeRoot: root, mode: "cloud" });
    expect(box.mode).toBe("cloud");
    await expect(box.snapshot()).rejects.toMatchObject({ code: "cloud_unsupported" });
    await expect(box.cd("foo")).rejects.toMatchObject({ code: "cloud_unsupported" });
  });
});
