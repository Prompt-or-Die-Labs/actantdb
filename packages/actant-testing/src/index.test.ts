import { describe, expect, it } from "vitest";

import {
  AssertionError,
  createTestLedger,
  expectChainIntact,
  expectEventEmitted,
  expectEventNotEmitted,
  expectGuardVerdict,
  expectToolCompleted,
  findEvents,
  snapshotEvents,
} from "./index.js";

function seed(t: ReturnType<typeof createTestLedger>) {
  const runId = t.newRun();
  t.appendUserMessage(runId, "issue a refund of $42 to invoice INV-001");
  t.appendToolCallRequested(runId, {
    tool_call_id: "tc1",
    tool: "issue_refund",
    risk: "high",
    args: { invoice: "INV-001", amount_cents: 4200 },
  });
  t.appendGuardVerdict(runId, "tc1", {
    decision: "require_approval",
    policy_snapshot: "p-hash",
    reason: "refund > $20 requires human approval",
  });
  t.appendApprovalRequired(runId, {
    tool_call_id: "tc1",
    tool: "issue_refund",
    reason: "refund > $20 requires human approval",
    args: { invoice: "INV-001", amount_cents: 4200 },
  });
  t.appendApprovalDecision(runId, "tc1", {
    decision: "approve",
    approver: "agent-test",
    scope: "single-call",
  });
  t.appendToolCallCompleted(runId, {
    tool_call_id: "tc1",
    duration_ms: 12,
    status: "ok",
    result: { refunded_cents: 4200 },
  });
  return runId;
}

describe("@actantdb/testing", () => {
  it("createTestLedger returns an in-memory ledger", () => {
    const t = createTestLedger({ project: "p" });
    expect(t.ledger.path()).toBe(":memory:");
    expect(t.events().length).toBe(0);
    t.close();
  });

  it("expectEventEmitted matches by kind and payload subset", () => {
    const t = createTestLedger();
    seed(t);
    const ev = expectEventEmitted(t, "tool_call_completed", {
      tool_call_id: "tc1",
      status: "ok",
    });
    expect(ev.kind).toBe("tool_call_completed");
    t.close();
  });

  it("expectEventEmitted throws when nothing matches, with helpful summary", () => {
    const t = createTestLedger();
    seed(t);
    expect(() => expectEventEmitted(t, "tool_call_completed", { status: "error" })).toThrow(
      AssertionError,
    );
    t.close();
  });

  it("expectEventNotEmitted is the inverse", () => {
    const t = createTestLedger();
    seed(t);
    expectEventNotEmitted(t, "tool_call_completed", { status: "error" });
    expect(() => expectEventNotEmitted(t, "tool_call_completed", { status: "ok" })).toThrow(
      AssertionError,
    );
    t.close();
  });

  it("expectGuardVerdict matches by tool_name + decision", () => {
    const t = createTestLedger();
    seed(t);
    const v = expectGuardVerdict(t, {
      tool_name: "issue_refund",
      decision: "require_approval",
    });
    expect(v.kind).toBe("guard_verdict");
    expect(() =>
      expectGuardVerdict(t, { tool_name: "issue_refund", decision: "allow" }),
    ).toThrow(AssertionError);
    t.close();
  });

  it("expectToolCompleted matches by tool_name + status", () => {
    const t = createTestLedger();
    seed(t);
    const v = expectToolCompleted(t, { tool_name: "issue_refund", status: "ok" });
    expect(v.kind).toBe("tool_call_completed");
    expect(() => expectToolCompleted(t, { tool_name: "issue_refund", status: "error" })).toThrow(
      AssertionError,
    );
    t.close();
  });

  it("expectChainIntact validates payload + chain hashes on every event", () => {
    const t = createTestLedger();
    seed(t);
    expectChainIntact(t);
    t.close();
  });

  it("findEvents returns all matches (not just the first)", () => {
    const t = createTestLedger();
    const r1 = t.newRun();
    t.appendUserMessage(r1, "one");
    t.appendUserMessage(r1, "two");
    t.appendUserMessage(r1, "three");
    const msgs = findEvents(t, "user_message_received");
    expect(msgs.length).toBe(3);
    t.close();
  });

  it("snapshotEvents strips churn-y keys for stable comparison", () => {
    const t = createTestLedger({ project: "snap" });
    const runId = t.newRun();
    t.appendUserMessage(runId, "hi");
    const snap = snapshotEvents(t.events());
    expect(snap.length).toBe(2);
    expect(snap[0]?.kind).toBe("agent_run_started");
    expect(snap[1]?.kind).toBe("user_message_received");
    // duration_ms is in the default scrub list; appended payloads here have
    // no such field, so we add one then verify it gets stripped.
    const tcRun = t.newRun();
    t.appendToolCallCompleted(tcRun, {
      tool_call_id: "tc",
      duration_ms: 123,
      status: "ok",
      result: { k: "v" },
    });
    const snap2 = snapshotEvents(t.events());
    const completed = snap2.find((s) => s.kind === "tool_call_completed");
    expect(completed?.payload).toBeDefined();
    expect((completed!.payload as Record<string, unknown>).duration_ms).toBeUndefined();
    expect((completed!.payload as Record<string, unknown>).status).toBe("ok");
    t.close();
  });

  it("accepts a bare Ledger, a TestLedger, or an event array", () => {
    const t = createTestLedger();
    seed(t);
    // bare ledger
    expectEventEmitted(t.ledger, "tool_call_completed");
    // array
    expectEventEmitted(t.events(), "tool_call_completed");
    t.close();
  });
});
