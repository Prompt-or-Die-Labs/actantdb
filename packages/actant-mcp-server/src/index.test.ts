import { describe, expect, it } from "vitest";

import { Ledger, buildContextManifest, ulid } from "@actantdb/core";
import type { ApprovalRequest, ContextManifest, ModelCall, PolicyVerdict, ToolCallCompleted, ToolCallRequest } from "@actantdb/types";

import { evaluatePredicate } from "./predicate.js";
import { parseActantUri, buildResources } from "./resources.js";
import { buildTools } from "./tools.js";
import { WorkspaceRegistry } from "./workspaces.js";

/** Build a fresh in-memory ledger + a minimal `RunContext`-like shim. */
function freshActant(project: string) {
  const ledger = new Ledger({ project, inMemory: true });
  return {
    ledger,
    close: () => ledger.close(),
    startRun(opts?: { meta?: unknown }) {
      const runId = ulid();
      ledger.append({
        kind: "agent_run_started",
        runId,
        payload: { project, meta: opts?.meta ?? null },
        sensitivity: "low",
      });
      return {
        runId,
        recordUserMessage: (text: string) =>
          ledger.append({ kind: "user_message_received", runId, payload: { text }, sensitivity: "low" }),
        recordContextBuild: (manifest: ContextManifest) =>
          ledger.append({ kind: "context_build", runId, payload: manifest, sensitivity: "medium" }),
        recordModelCall: (info: ModelCall) =>
          ledger.append({ kind: "model_call", runId, payload: info, sensitivity: "low" }),
        recordToolCallRequested: (req: ToolCallRequest) =>
          ledger.append({ kind: "tool_call_requested", runId, payload: req, sensitivity: "low" }),
        recordGuardVerdict: (toolCallId: string, verdict: PolicyVerdict) =>
          ledger.append({
            kind: "guard_verdict",
            runId,
            payload: { tool_call_id: toolCallId, ...verdict },
            sensitivity: "low",
          }),
        recordApprovalRequired: (req: ApprovalRequest) =>
          ledger.append({ kind: "approval_required", runId, payload: req, sensitivity: "low" }),
        recordToolCallStarted: (toolCallId: string, finalArgs: unknown) =>
          ledger.append({
            kind: "tool_call_started",
            runId,
            payload: { tool_call_id: toolCallId, final_args: finalArgs },
            sensitivity: "low",
          }),
        recordToolCallCompleted: (payload: ToolCallCompleted) =>
          ledger.append({ kind: "tool_call_completed", runId, payload, sensitivity: "low" }),
        finish: (payload: unknown) =>
          ledger.append({ kind: "agent_run_finished", runId, payload: payload ?? {}, sensitivity: "low" }),
      };
    },
  };
}

function seedRun(actant: ReturnType<typeof freshActant>) {
  const ctx = actant.startRun();
  ctx.recordUserMessage("hello");
  ctx.recordContextBuild(
    buildContextManifest([
      {
        id: "mem_1",
        kind: "memory",
        source: "test",
        sensitivity: "low",
        label: "m",
        content: "hi",
      },
    ]),
  );
  const planner = ctx.recordModelCall({
    model: "noop",
    role: "planner",
    prompt_hash: "h",
    summary: "shell.run rm tmp",
  });
  ctx.recordToolCallRequested({
    tool_call_id: "tc1",
    tool: "shell.run",
    risk: "low",
    args: { command: "rm tmp" },
  });
  ctx.recordGuardVerdict("tc1", {
    decision: "require_approval",
    policy_snapshot: "p",
    reason: "test policy needs approval",
  });
  ctx.recordApprovalRequired({
    tool_call_id: "tc1",
    tool: "shell.run",
    reason: "test policy needs approval",
    args: { command: "rm tmp" },
  });
  ctx.recordToolCallStarted("tc1", { command: "rm tmp" });
  ctx.recordToolCallCompleted({
    tool_call_id: "tc1",
    duration_ms: 1,
    status: "ok",
    result: { stdout: "" },
  });
  ctx.finish({ ok: true });
  return { runId: ctx.runId, plannerEventId: planner.id };
}

describe("predicate evaluator", () => {
  it("matches eq on dotted paths", () => {
    const root = { payload: { status: "ok", n: 3 } };
    expect(evaluatePredicate({ op: "eq", field: "payload.status", value: "ok" }, root)).toBe(true);
    expect(evaluatePredicate({ op: "eq", field: "payload.status", value: "no" }, root)).toBe(false);
  });

  it("treats missing fields as false except for ne", () => {
    const root = { payload: {} };
    expect(evaluatePredicate({ op: "eq", field: "payload.x", value: 1 }, root)).toBe(false);
    expect(evaluatePredicate({ op: "ne", field: "payload.x", value: 1 }, root)).toBe(true);
    expect(evaluatePredicate({ op: "exists", field: "payload.x" }, root)).toBe(false);
  });

  it("supports numeric and boolean ordering, plus and/or/not", () => {
    const root = { payload: { n: 3, ok: true } };
    expect(evaluatePredicate({ op: "gt", field: "payload.n", value: 2 }, root)).toBe(true);
    expect(evaluatePredicate({ op: "le", field: "payload.n", value: 3 }, root)).toBe(true);
    expect(
      evaluatePredicate(
        {
          op: "and",
          args: [
            { op: "eq", field: "payload.ok", value: true },
            { op: "or", args: [{ op: "lt", field: "payload.n", value: 0 }, { op: "ge", field: "payload.n", value: 3 }] },
            { op: "not", arg: { op: "eq", field: "payload.n", value: 99 } },
          ],
        },
        root,
      ),
    ).toBe(true);
  });

  it("indexes into arrays via numeric path segments", () => {
    const root = { payload: { items: [{ id: "a" }, { id: "b" }] } };
    expect(evaluatePredicate({ op: "eq", field: "payload.items.1.id", value: "b" }, root)).toBe(true);
    expect(evaluatePredicate({ op: "eq", field: "payload.items.5.id", value: "x" }, root)).toBe(false);
  });
});

describe("parseActantUri", () => {
  it("parses session and runs URIs", () => {
    expect(parseActantUri("actant://workspace/ws1/runs")).toEqual({
      kind: "runs",
      workspaceId: "ws1",
    });
    expect(parseActantUri("actant://workspace/ws1/session/run-7")).toEqual({
      kind: "session",
      workspaceId: "ws1",
      sessionId: "run-7",
    });
    expect(parseActantUri("file:///tmp/x")).toBeNull();
  });
});

describe("MCP tool surface", () => {
  it("lists runs and gets events from a shared in-memory ledger", async () => {
    const actant = freshActant("ws-x");
    // Use freshActant so each test runs against an isolated in-memory ledger.
    const registry = new WorkspaceRegistry({ sharedLedger: actant.ledger });
    const tools = buildTools(registry);

    const { runId } = seedRun(actant);

    const runs = await tools.listRuns({ workspace_id: "ws-x" });
    expect(runs.runs.length).toBe(1);
    expect(runs.runs[0]?.run_id).toBe(runId);

    const events = await tools.listEvents({
      workspace_id: "ws-x",
      session_id: runId,
      limit: 100,
    });
    expect(events.events.length).toBeGreaterThan(5);
    expect(events.events.map((e) => e.kind)).toContain("tool_call_completed");

    const first = events.events[0]!;
    const got = await tools.getEvent({ workspace_id: "ws-x", event_id: first.id });
    expect(got.event?.id).toBe(first.id);

    const summary = await tools.getWorkspaceSummary({ workspace_id: "ws-x" });
    expect(summary.session_count).toBe(1);
    expect(summary.event_count).toBeGreaterThan(5);

    actant.close();
  });

  it("query_predicate filters by predicate AST", async () => {
    const actant = freshActant("ws-q");
    seedRun(actant);
    const registry = new WorkspaceRegistry({ sharedLedger: actant.ledger });
    const tools = buildTools(registry);

    const out = await tools.queryPredicate({
      workspace_id: "ws-q",
      topic: "tool_call_completed",
      predicate_expr: { op: "eq", field: "payload.status", value: "ok" },
    });
    expect(out.matches.length).toBe(1);
    expect(out.matches[0]?.kind).toBe("tool_call_completed");

    actant.close();
  });

  it("decide_approval records an approval_decision and clears pending", async () => {
    const actant = freshActant("ws-a");
    seedRun(actant);
    const registry = new WorkspaceRegistry({ sharedLedger: actant.ledger });
    const tools = buildTools(registry);

    const pending = await tools.listPendingApprovals({ workspace_id: "ws-a" });
    expect(pending.pending.length).toBe(1);
    expect(pending.pending[0]?.tool_call_id).toBe("tc1");

    const dec = await tools.decideApproval({
      workspace_id: "ws-a",
      request_id: "tc1",
      decision: "approve",
      approver: "tester",
    });
    expect(dec.status).toBe("recorded");

    const after = await tools.listPendingApprovals({ workspace_id: "ws-a" });
    expect(after.pending.length).toBe(0);

    actant.close();
  });

  it("replay returns a ReplayDiff against the original recorded run", async () => {
    const actant = freshActant("ws-r");
    const { plannerEventId } = seedRun(actant);
    const registry = new WorkspaceRegistry({ sharedLedger: actant.ledger });
    const tools = buildTools(registry);

    const out = await tools.replay({
      workspace_id: "ws-r",
      event_id: plannerEventId,
      overrides: {},
    });
    expect(out.diff.entries.length).toBeGreaterThan(0);
    actant.close();
  });

  it("resources return JSON payloads for the right URI templates", async () => {
    const actant = freshActant("ws-res");
    const { runId } = seedRun(actant);
    const registry = new WorkspaceRegistry({ sharedLedger: actant.ledger });
    const resources = buildResources(registry);

    const session = resources.readSession("ws-res", runId);
    expect(session.mimeType).toBe("application/json");
    const parsed = JSON.parse(session.text);
    expect(parsed.session_id).toBe(runId);
    expect(Array.isArray(parsed.events)).toBe(true);

    const runs = resources.readRuns("ws-res");
    const runsParsed = JSON.parse(runs.text);
    expect(runsParsed.runs.length).toBe(1);

    actant.close();
  });
});
