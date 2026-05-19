/**
 * Tests for `ctx.run / sleep / sleepUntil / waitForEvent / call`.
 *
 * Every test uses an in-memory ledger so we don't touch disk and don't
 * collide across tests. The clock is mocked with a closure-over-`now`
 * variable injected into the runner state — easier to read than vitest's
 * fake timers and lets us prove "sleep resumes when deadline passes" with
 * deterministic numbers.
 */

import { describe, expect, it, vi } from "vitest";

import { openLedger } from "@actantdb/core";

import { WorkflowContext } from "./context.js";
import { WorkflowSuspended } from "./errors.js";
import { makeState, recordNotify } from "./runner.js";

interface Harness {
  ctx: WorkflowContext;
  setNow: (ms: number) => void;
  /** Fresh state simulating a new HTTP invocation. */
  reset: () => void;
  runId: string;
}

function freshCtx(opts: { payload?: unknown; fetch?: typeof globalThis.fetch } = {}): Harness {
  const ledger = openLedger({ project: "wf-test", inMemory: true });
  const runId = "wfr_test";
  let now = 1_000_000;
  let state = makeState(ledger, runId, {
    now: () => now,
    ...(opts.fetch ? { fetch: opts.fetch } : {}),
  });
  let ctx = new WorkflowContext({
    runId,
    payload: opts.payload ?? null,
    state,
    ledger,
  });
  return {
    get ctx() {
      return ctx;
    },
    setNow(ms: number) {
      now = ms;
    },
    reset() {
      state = makeState(ledger, runId, {
        now: () => now,
        ...(opts.fetch ? { fetch: opts.fetch } : {}),
      });
      ctx = new WorkflowContext({
        runId,
        payload: opts.payload ?? null,
        state,
        ledger,
      });
    },
    runId,
  } as Harness;
}

describe("ctx.run", () => {
  it("invokes fn the first time and caches the result on resume", async () => {
    const h = freshCtx();
    const fn = vi.fn().mockResolvedValue(42);

    // First invocation: fn runs.
    const a = await h.ctx.run("fetch", fn);
    expect(a).toBe(42);
    expect(fn).toHaveBeenCalledTimes(1);

    // Simulate a new HTTP invocation by rebuilding state.
    h.reset();
    const b = await h.ctx.run("fetch", fn);
    expect(b).toBe(42);
    // fn must NOT have been called again.
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it("throws on duplicate step names within one invocation", async () => {
    const h = freshCtx();
    await h.ctx.run("x", async () => 1);
    await expect(h.ctx.run("x", async () => 2)).rejects.toThrow(/duplicate step/);
  });
});

describe("ctx.sleep", () => {
  it("suspends on first encounter, resumes when the deadline passes", () => {
    const h = freshCtx();
    h.setNow(0);

    // First time → marker written, suspension thrown with retryAfterMs.
    let suspended: WorkflowSuspended | undefined;
    try {
      h.ctx.sleep("grace", "5m");
    } catch (e) {
      suspended = e as WorkflowSuspended;
    }
    expect(suspended).toBeInstanceOf(WorkflowSuspended);
    expect(suspended!.retryAfterMs).toBe(5 * 60_000);

    // Resume before deadline → suspended again.
    h.reset();
    h.setNow(60_000); // 1 min in
    expect(() => h.ctx.sleep("grace", "5m")).toThrow(WorkflowSuspended);

    // Resume after deadline → returns cleanly.
    h.reset();
    h.setNow(10 * 60_000); // 10 min in
    expect(() => h.ctx.sleep("grace", "5m")).not.toThrow();
  });
});

describe("ctx.sleepUntil", () => {
  it("honors an absolute ISO timestamp", () => {
    const h = freshCtx();
    h.setNow(Date.parse("2026-01-01T00:00:00Z"));
    expect(() => h.ctx.sleepUntil("nye", "2026-01-02T00:00:00Z")).toThrow(
      WorkflowSuspended,
    );

    // Advance the clock past the deadline; new invocation should pass through.
    h.reset();
    h.setNow(Date.parse("2026-01-03T00:00:00Z"));
    expect(() => h.ctx.sleepUntil("nye", "2026-01-02T00:00:00Z")).not.toThrow();
  });
});

describe("ctx.waitForEvent", () => {
  it("suspends until a matching notify lands, then returns the data", () => {
    const ledger = openLedger({ project: "wf-wait", inMemory: true });
    const runId = "wfr_wait";
    let now = 0;
    const mk = () =>
      new WorkflowContext({
        runId,
        payload: null,
        state: makeState(ledger, runId, { now: () => now }),
        ledger,
      });

    // First invocation: suspends.
    expect(() => mk().waitForEvent("ship", "shipped:42")).toThrow(WorkflowSuspended);

    // External notify lands.
    recordNotify(ledger, runId, "shipped:42", { tracking: "abc" });

    // Next invocation: returns the payload.
    const data = mk().waitForEvent<{ tracking: string }>("ship", "shipped:42");
    expect(data).toEqual({ tracking: "abc" });
  });

  it("honors timeout — returns undefined after the deadline", () => {
    const ledger = openLedger({ project: "wf-wait-to", inMemory: true });
    const runId = "wfr_to";
    let now = 0;
    const mk = () =>
      new WorkflowContext({
        runId,
        payload: null,
        state: makeState(ledger, runId, { now: () => now }),
        ledger,
      });

    expect(() => mk().waitForEvent("x", "evt", { timeout: "5m" })).toThrow(
      WorkflowSuspended,
    );

    // 10 min later, no notify → resolves to undefined.
    now = 10 * 60_000;
    const data = mk().waitForEvent("x", "evt", { timeout: "5m" });
    expect(data).toBeUndefined();
  });
});

describe("ctx.call", () => {
  it("issues the fetch once and caches the response", async () => {
    const responseBody = { ok: true };
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify(responseBody), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    const h = freshCtx({ fetch: fetchMock as unknown as typeof globalThis.fetch });

    const r1 = await h.ctx.call<{ ok: boolean }>("ping", {
      url: "https://example.com/ping",
      method: "POST",
      body: { hello: "world" },
    });
    expect(r1.status).toBe(200);
    expect(r1.body).toEqual(responseBody);
    expect(fetchMock).toHaveBeenCalledTimes(1);

    // Resume: cached.
    h.reset();
    const r2 = await h.ctx.call<{ ok: boolean }>("ping", {
      url: "https://example.com/ping",
      method: "POST",
      body: { hello: "world" },
    });
    expect(r2.body).toEqual(responseBody);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
