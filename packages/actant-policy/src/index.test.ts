import { describe, expect, it } from "vitest";

import {
  demoPolicy,
  evaluate,
  requiresApproval,
  riskOf,
  sensitivityExceeds,
  snapshotHash,
  verdict,
} from "./index.js";

describe("policy verdict builders", () => {
  it("constructs an allow verdict", () => {
    const v = verdict.allow("ok", "snap");
    expect(v.decision).toBe("allow");
  });
  it("constructs a constrain verdict", () => {
    const v = verdict.constrain("rewrite", "snap", { x: 1 }, "drop x");
    if (v.decision !== "constrain") throw new Error("kind");
    expect(v.constrained_input).toEqual({ x: 1 });
    expect(v.hint).toBe("drop x");
  });
});

describe("policy helpers", () => {
  it("riskOf returns low by default", () => {
    expect(riskOf({ tools: [] }, "file.read")).toBe("low");
  });
  it("requiresApproval reads the entry", () => {
    expect(requiresApproval(demoPolicy, "shell.run")).toBe(true);
    expect(requiresApproval(demoPolicy, "file.read")).toBe(false);
  });
  it("sensitivityExceeds is ordered", () => {
    expect(sensitivityExceeds("high", "medium")).toBe(true);
    expect(sensitivityExceeds("low", "medium")).toBe(false);
  });
  it("snapshotHash is stable for same policy", () => {
    expect(snapshotHash(demoPolicy)).toBe(snapshotHash(demoPolicy));
  });
});

describe("policy evaluate", () => {
  const snap = snapshotHash(demoPolicy);

  it("blocks/constrain hints rm -rf with /dist via deny + shell default", () => {
    const v = evaluate(demoPolicy, {
      tool_call_id: "t1",
      tool: "shell.run",
      args: { command: "rm -rf build dist" },
      risk: "destructive",
    });
    expect(v.decision).toBe("require_approval");
    expect(snap).toMatch(/^[0-9a-f]{64}$/);
    if (v.decision === "require_approval") {
      expect(v.constrained_input).toBeDefined();
      const newArgs = v.constrained_input as { command: string };
      expect(newArgs.command).toBe("rm -rf build");
      expect(v.hint).toContain("drop");
    }
  });

  it("approves low-risk reads", () => {
    const v = evaluate(demoPolicy, {
      tool_call_id: "t2",
      tool: "file.read",
      args: { path: "README" },
      risk: "low",
    });
    expect(v.decision).toBe("allow");
  });

  it("require_approval for shell.run without dangerous arg", () => {
    const v = evaluate(demoPolicy, {
      tool_call_id: "t3",
      tool: "shell.run",
      args: { command: "ls -la" },
      risk: "low",
    });
    expect(v.decision).toBe("require_approval");
  });
});
