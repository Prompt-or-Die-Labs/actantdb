import { describe, expect, it } from "vitest";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { buildContextManifest, createActant } from "@actantdb/core";
import { demoPolicy } from "@actantdb/policy";

import { diff, diffReplayAgainstOriginal, runFromEvent, tighten } from "./index.js";

function freshDir(): string {
  return mkdtempSync(join(tmpdir(), "actantdb-replay-test-"));
}

describe("replay (memory + policy modes)", () => {
  it("rebuilds the manifest minus excluded memory ids", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "r", storeDir: dir, policy: demoPolicy });
      const ctx = a.startRun();
      ctx.recordContextBuild(
        buildContextManifest([
          { id: "mem_42_dist", kind: "memory", source: "m", sensitivity: "low", label: "stale", content: "build artifacts under /build and /dist" },
          { id: "doc_readme", kind: "document", source: "f", sensitivity: "public", label: "readme", content: "readme text" },
        ]),
      );
      const planner = ctx.recordModelCall({ model: "x", role: "p", prompt_hash: "h", summary: "rm -rf build dist" });
      ctx.recordToolCallRequested({
        tool_call_id: "tc1",
        tool: "shell.run",
        args: { command: "rm -rf build dist" },
        risk: "destructive",
      });
      ctx.recordGuardVerdict("tc1", {
        decision: "require_approval",
        reason: "shell.run rm -rf with /dist",
        policy_snapshot: "snap",
      });
      const replay = runFromEvent({
        ledger: a.ledger,
        eventId: planner.id,
        overrides: { without_memory: ["mem_42_dist"] },
        policy: tighten(demoPolicy, {
          deny: [
            { tool: "shell.run", pattern: "\\bdist\\b", reason: "no dist" },
          ],
        }),
        alternatePlannerOutput: "rm -rf build",
      });
      const toolReq = replay.events.find((e) => e.kind === "tool_call_requested");
      expect(toolReq).toBeDefined();
      const args = (toolReq!.payload as { args: { command: string } }).args;
      expect(args.command).toBe("rm -rf build");
      const dif = diffReplayAgainstOriginal(a.ledger, replay);
      expect(dif.entries.length).toBeGreaterThan(0);
      expect(dif.entries.some((d) => d.diff === "changed")).toBe(true);
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("diff reports identical when streams match exactly", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "rr", storeDir: dir });
      const ctx = a.startRun();
      ctx.recordUserMessage("hi");
      const events = a.ledger.query({ runId: ctx.runId });
      const dif = diff(events, events);
      for (const e of dif.entries) expect(e.diff).toBe("identical");
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

// ---------------------------------------------------------------------------
// Mode-specific tests for the three modes added in GAPS.md row #7 closure.
// Each scenario mirrors the Rust side in crates/actant-replay/tests/*.rs.
// ---------------------------------------------------------------------------

describe("replay mode=tool (substitution)", () => {
  it("substitutes the designated tool call, leaves others alone", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "tsub", storeDir: dir });
      const ctx = a.startRun();
      const anchor = ctx.recordModelCall({
        model: "x",
        role: "planner",
        prompt_hash: "h",
        summary: "open both files",
      });
      ctx.recordToolCallRequested({
        tool_call_id: "tc_a",
        tool: "fs.read",
        args: { path: "a.txt" },
        risk: "low",
      });
      ctx.recordToolCallCompleted({
        tool_call_id: "tc_a",
        result: "old-a",
        status: "ok",
        duration_ms: 1,
      });
      ctx.recordToolCallRequested({
        tool_call_id: "tc_b",
        tool: "fs.read",
        args: { path: "b.txt" },
        risk: "low",
      });
      ctx.recordToolCallCompleted({
        tool_call_id: "tc_b",
        result: "old-b",
        status: "ok",
        duration_ms: 1,
      });
      ctx.recordModelCall({
        model: "x",
        role: "summarizer",
        prompt_hash: "h2",
        summary: "wrap up",
      });

      const replay = runFromEvent({
        ledger: a.ledger,
        eventId: anchor.id,
        mode: "tool",
        toolSubstitutions: { tc_a: "NEW-A" },
      });

      const completions = replay.events.filter(
        (e) => e.kind === "tool_call_completed",
      );
      const a_comp = completions.find(
        (e) => (e.payload as { tool_call_id: string }).tool_call_id === "tc_a",
      )!;
      expect((a_comp.payload as { result: unknown }).result).toBe("NEW-A");
      expect((a_comp.payload as { status: string }).status).toBe("substituted");

      const b_comp = completions.find(
        (e) => (e.payload as { tool_call_id: string }).tool_call_id === "tc_b",
      )!;
      expect((b_comp.payload as { result: unknown }).result).toBe("old-b");

      const dif = diffReplayAgainstOriginal(a.ledger, replay);
      const a_diff = dif.entries.find(
        (e) =>
          e.kind === "tool_call_completed" &&
          ((e.b as { tool_call_id: string } | undefined)?.tool_call_id === "tc_a"),
      );
      expect(a_diff?.diff).toBe("changed");
      const b_diff = dif.entries.find(
        (e) =>
          e.kind === "tool_call_completed" &&
          ((e.b as { tool_call_id: string } | undefined)?.tool_call_id === "tc_b"),
      );
      expect(b_diff?.diff).toBe("identical");

      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("empty substitutions -> every event identical", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "tsub-empty", storeDir: dir });
      const ctx = a.startRun();
      const anchor = ctx.recordModelCall({
        model: "x",
        role: "planner",
        prompt_hash: "h",
        summary: "ok",
      });
      ctx.recordToolCallRequested({
        tool_call_id: "tc1",
        tool: "fs.read",
        args: { path: "x" },
        risk: "low",
      });
      ctx.recordToolCallCompleted({
        tool_call_id: "tc1",
        result: "x",
        status: "ok",
        duration_ms: 1,
      });
      const replay = runFromEvent({
        ledger: a.ledger,
        eventId: anchor.id,
        mode: "tool",
      });
      const dif = diffReplayAgainstOriginal(a.ledger, replay);
      for (const e of dif.entries) expect(e.diff).toBe("identical");
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("replay mode=experimental", () => {
  it("re-invokes a single tool with the supplied replacement; downstream is changed", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "exp", storeDir: dir });
      const ctx = a.startRun();
      const anchor = ctx.recordModelCall({
        model: "x",
        role: "planner",
        prompt_hash: "h",
        summary: "ask weather then summarise",
      });
      ctx.recordToolCallRequested({
        tool_call_id: "tc_weather",
        tool: "http.get",
        args: { url: "https://wttr.in" },
        risk: "low",
      });
      ctx.recordToolCallCompleted({
        tool_call_id: "tc_weather",
        result: { temp: 68 },
        status: "ok",
        duration_ms: 5,
      });
      ctx.recordModelCall({
        model: "x",
        role: "summarizer",
        prompt_hash: "h2",
        summary: "summarise: 68",
      });
      ctx.finish();

      const replay = runFromEvent({
        ledger: a.ledger,
        eventId: anchor.id,
        mode: "experimental",
        experimentalToolCallId: "tc_weather",
        experimentalReplacementResult: { temp: 72 },
      });

      const reinvoked = replay.events.find(
        (e) =>
          e.kind === "tool_call_completed" &&
          (e.payload as { tool_call_id: string }).tool_call_id === "tc_weather",
      )!;
      expect((reinvoked.payload as { status: string }).status).toBe("reinvoked");
      expect(
        ((reinvoked.payload as { result: { temp: number } }).result).temp,
      ).toBe(72);

      const dif = diffReplayAgainstOriginal(a.ledger, replay);
      const reinvDiff = dif.entries.find(
        (e) =>
          e.kind === "tool_call_completed" &&
          ((e.b as { tool_call_id: string } | undefined)?.tool_call_id === "tc_weather"),
      );
      expect(reinvDiff?.diff).toBe("changed");
      const downstreamModelCalls = dif.entries.filter(
        (e) => e.kind === "model_call",
      );
      expect(downstreamModelCalls.some((d) => d.diff === "identical")).toBe(true);
      expect(downstreamModelCalls.some((d) => d.diff === "changed")).toBe(true);

      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("throws when the tool_call_id is not in the recorded run", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "exp-missing", storeDir: dir });
      const ctx = a.startRun();
      const anchor = ctx.recordModelCall({
        model: "x",
        role: "p",
        prompt_hash: "h",
        summary: "ok",
      });
      expect(() =>
        runFromEvent({
          ledger: a.ledger,
          eventId: anchor.id,
          mode: "experimental",
          experimentalToolCallId: "does-not-exist",
        }),
      ).toThrow(/does-not-exist/);
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("replay mode=local_only", () => {
  it("matches recorded shape for a run with no remote routes", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "lo", storeDir: dir });
      const ctx = a.startRun();
      const anchor = ctx.recordModelCall({
        model: "local:ollama:llama3",
        role: "planner",
        prompt_hash: "h",
        summary: "local-only ok",
      });
      ctx.recordToolCallRequested({
        tool_call_id: "tc1",
        tool: "fs.read",
        args: { path: "x" },
        risk: "low",
      });
      ctx.recordToolCallCompleted({
        tool_call_id: "tc1",
        result: "x",
        status: "ok",
        duration_ms: 1,
      });
      ctx.finish();
      const replayLocal = runFromEvent({
        ledger: a.ledger,
        eventId: anchor.id,
        mode: "local_only",
      });
      const difLocal = diffReplayAgainstOriginal(a.ledger, replayLocal);
      for (const e of difLocal.entries) {
        expect(e.diff).toBe("identical");
      }
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("leaves local model_calls identical even with a cloud sibling", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "lo-cloud", storeDir: dir });
      const ctx = a.startRun();
      const anchor = ctx.recordModelCall({
        model: "anthropic:claude-opus-4-7",
        role: "planner",
        prompt_hash: "h",
        summary: "expensive plan",
      });
      ctx.recordModelCall({
        model: "local:ollama",
        role: "summarizer",
        prompt_hash: "h2",
        summary: "cheap follow-up",
      });
      ctx.finish();

      const replay = runFromEvent({
        ledger: a.ledger,
        eventId: anchor.id,
        mode: "local_only",
      });
      const dif = diffReplayAgainstOriginal(a.ledger, replay);
      const modelCalls = dif.entries.filter((e) => e.kind === "model_call");
      expect(modelCalls.length).toBe(2);
      const local = modelCalls.find(
        (e) =>
          (e.b as { model: string } | undefined)?.model === "local:ollama",
      );
      expect(local?.diff).toBe("identical");
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it("is deterministic (same input -> same payload-hash shape)", () => {
    const dir = freshDir();
    try {
      const a = createActant({ project: "lo-det", storeDir: dir });
      const ctx = a.startRun();
      const anchor = ctx.recordModelCall({
        model: "anthropic:claude",
        role: "p",
        prompt_hash: "h",
        summary: "x",
      });
      ctx.recordModelCall({
        model: "local:ollama",
        role: "p",
        prompt_hash: "h2",
        summary: "y",
      });
      const r1 = runFromEvent({
        ledger: a.ledger,
        eventId: anchor.id,
        mode: "local_only",
      });
      const r2 = runFromEvent({
        ledger: a.ledger,
        eventId: anchor.id,
        mode: "local_only",
      });
      expect(r1.id).not.toBe(r2.id);
      expect(r1.events.length).toBe(r2.events.length);
      for (let i = 0; i < r1.events.length; i++) {
        expect(r1.events[i]!.kind).toBe(r2.events[i]!.kind);
        expect(r1.events[i]!.payload_hash).toBe(r2.events[i]!.payload_hash);
      }
      a.close();
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
