import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { Box } from "./index.js";
import type { ExecChunk } from "./types.js";

let root: string;

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "actantdb-box-exec-"));
});

afterEach(() => {
  rmSync(root, { recursive: true, force: true });
});

describe("box.exec", () => {
  it("command captures stdout and records a tool_call_completed event", async () => {
    const box = await Box.create({ storeRoot: root });
    const run = await box.exec.command("echo hi");
    const result = run.result as { exit: number; output: string };
    expect(result.exit).toBe(0);
    expect(result.output.trim()).toBe("hi");
    expect(run.status).toBe("ok");

    const events = box.ledger.query({});
    const completed = events.find(
      (e) =>
        e.kind === "tool_call_completed" &&
        (e.payload as { tool_call_id?: string }).tool_call_id,
    );
    expect(completed).toBeDefined();

    const effect = events.find(
      (e) => e.kind === "effect_observed" && (e.payload as { kind?: string }).kind === "exec_completed",
    );
    expect(effect).toBeDefined();
    await box.delete();
  });

  it("stream yields stdout chunks then an exit chunk", async () => {
    const box = await Box.create({ storeRoot: root });
    const chunks: ExecChunk[] = [];
    for await (const c of box.exec.stream("printf 'a\\nb\\n'")) {
      chunks.push(c);
    }
    const stdout = chunks.filter((c) => c.type === "stdout").map((c) => c.line);
    expect(stdout).toEqual(["a", "b"]);
    expect(chunks[chunks.length - 1]!.type).toBe("exit");
    await box.delete();
  });

  it("nonzero exit becomes status='error' but does not throw", async () => {
    const box = await Box.create({ storeRoot: root });
    const run = await box.exec.command("sh -c 'exit 7'");
    expect(run.status).toBe("error");
    const result = run.result as { exit: number };
    expect(result.exit).toBe(7);
    await box.delete();
  });
});
