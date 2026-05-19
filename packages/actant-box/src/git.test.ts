import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { Box } from "./index.js";

let root: string;

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "actantdb-box-git-"));
});

afterEach(() => {
  rmSync(root, { recursive: true, force: true });
});

function gitAvailable(): boolean {
  const r = spawnSync("git", ["--version"]);
  return r.status === 0;
}

describe("box.git", () => {
  it.skipIf(!gitAvailable())("init + add + commit + status round-trips locally", async () => {
    const box = await Box.create({ storeRoot: root });
    // init a repo in the workspace
    await box.git.exec({ args: ["init", "-q"] });
    await box.git.updateConfig({ userName: "Test", userEmail: "t@example.com" });
    await box.files.write({ path: "README.md", content: "hello" });
    await box.git.commit({ message: "initial" });
    const status = await box.git.status();
    expect(status.clean).toBe(true);

    // diff is empty after commit
    expect(await box.git.diff()).toBe("");

    // Modify + status surfaces the change
    await box.files.write({ path: "README.md", content: "hello-2" });
    const dirty = await box.git.status();
    expect(dirty.clean).toBe(false);
    expect(dirty.files.some((f) => f.path.includes("README.md"))).toBe(true);

    await box.delete();
  });

  it.skipIf(!gitAvailable())("createPR falls back to printing command when gh is missing", async () => {
    const box = await Box.create({ storeRoot: root });
    await box.git.exec({ args: ["init", "-q"] });
    // We don't actually push — we just exercise the createPR fallback path.
    // On CI without `gh`, submitted=false; with `gh` configured, it'd try to
    // run and likely fail because there's no remote. Either way command is set.
    let result: Awaited<ReturnType<typeof box.git.createPR>>;
    try {
      result = await box.git.createPR({ title: "demo", body: "x" });
    } catch (err) {
      // when `gh` is present but the repo has no remote/origin gh exits nonzero
      // and our wrapper rethrows BoxError("git_failed"); skip the assertion.
      expect((err as { code?: string }).code).toBe("git_failed");
      await box.delete();
      return;
    }
    expect(result.command).toContain("gh pr create");
    await box.delete();
  });
});
