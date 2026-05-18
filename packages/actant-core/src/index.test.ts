import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { canonicalJSON, sha256OfJSON, ulid } from "./index.js";
import { Ledger } from "./ledger.js";
import { createActant, buildContextManifest } from "./runtime.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-core-test-"));
}

describe("hash", () => {
  it("canonicalJSON is stable across key order", () => {
    expect(canonicalJSON({ a: 1, b: 2 })).toBe(canonicalJSON({ b: 2, a: 1 }));
    expect(canonicalJSON({ a: 1, b: { c: 3, d: 4 } })).toBe(
      canonicalJSON({ b: { d: 4, c: 3 }, a: 1 }),
    );
  });

  it("sha256OfJSON produces 64-hex digest", () => {
    expect(sha256OfJSON({ a: 1 })).toMatch(/^[0-9a-f]{64}$/);
  });
});

describe("ulid", () => {
  it("is monotonically ordered within the same ms", () => {
    const ts = Date.now();
    const ids = Array.from({ length: 50 }, () => ulid(ts));
    const sorted = [...ids].sort();
    expect(ids).toEqual(sorted);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("Ledger", () => {
  it("hash-chains events per run", () => {
    const dir = freshDir();
    try {
      const ledger = new Ledger({ project: "t", storeDir: dir });
      const runId = ulid();
      const a = ledger.append({ kind: "agent_run_started", runId, payload: { a: 1 } });
      const b = ledger.append({ kind: "user_message_received", runId, payload: { text: "hi" } });
      expect(b.chain_hash).not.toBe(a.chain_hash);
      // Each chain hash is reproducible from prev.
      const all = ledger.query({ runId });
      expect(all.length).toBe(2);
      expect(all[1]!.chain_hash).toBe(b.chain_hash);
      ledger.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("builds checkpoints that capture manifest + policy hashes", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "cp", storeDir: dir });
      const ctx = a.startRun();
      ctx.recordContextBuild(
        buildContextManifest([
          {
            id: "mem_42",
            kind: "memory",
            source: "m",
            sensitivity: "low",
            label: "x",
            content: "y",
          },
        ]),
      );
      const planner = ctx.recordModelCall({
        model: "noop",
        role: "planner",
        prompt_hash: "h",
        summary: "s",
      });
      const cp = a.ledger.checkpoint(planner.id);
      expect(cp.event_id).toBe(planner.id);
      expect(cp.manifest_hash).toMatch(/^[0-9a-f]{64}$/);
      expect(cp.memory_set_hash).toMatch(/^[0-9a-f]{64}$/);
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
