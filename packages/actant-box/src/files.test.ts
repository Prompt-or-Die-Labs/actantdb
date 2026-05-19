import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { Box } from "./index.js";

let root: string;

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "actantdb-box-files-"));
});

afterEach(() => {
  rmSync(root, { recursive: true, force: true });
});

describe("box.files", () => {
  it("write + read round-trips and records typed effect_observed events", async () => {
    const box = await Box.create({ storeRoot: root });
    await box.files.write({ path: "hello.txt", content: "world" });
    const out = await box.files.read("hello.txt");
    expect(out).toBe("world");

    const effects = box.ledger
      .query({})
      .map((e) => e.payload as { kind?: string })
      .filter((p) => p.kind === "file_write" || p.kind === "file_read");
    expect(effects.some((e) => e.kind === "file_write")).toBe(true);
    expect(effects.some((e) => e.kind === "file_read")).toBe(true);

    await box.delete();
  });

  it("list returns workspace-relative paths", async () => {
    const box = await Box.create({ storeRoot: root });
    await box.files.write({ path: "a.txt", content: "1" });
    await box.files.write({ path: "b.txt", content: "2" });
    const entries = await box.files.list();
    const names = entries.map((e) => e.path).sort();
    expect(names).toEqual(["a.txt", "b.txt"]);
    await box.delete();
  });

  it("upload copies a local file into the workspace", async () => {
    const box = await Box.create({ storeRoot: root });
    const src = join(root, "external.txt");
    writeFileSync(src, "from-host");
    await box.files.upload([{ path: src, destination: "imported.txt" }]);
    const out = await box.files.read("imported.txt");
    expect(out).toBe("from-host");
    await box.delete();
  });

  it("download copies workspace files out to a folder", async () => {
    const box = await Box.create({ storeRoot: root });
    await box.files.write({ path: "out.txt", content: "exported" });
    const downloadDir = join(root, "out");
    await box.files.download({ folder: downloadDir });
    const reread = (await import("node:fs")).readFileSync(join(downloadDir, "out.txt"), "utf8");
    expect(reread).toBe("exported");
    await box.delete();
  });

  it("write refuses paths that escape the workspace", async () => {
    const box = await Box.create({ storeRoot: root });
    await expect(
      box.files.write({ path: "../escape.txt", content: "no" }),
    ).rejects.toMatchObject({ code: "invalid_argument" });
    await box.delete();
  });
});
