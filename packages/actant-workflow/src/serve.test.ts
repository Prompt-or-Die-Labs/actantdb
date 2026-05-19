/**
 * Tests for the `serve()` HTTP factory and the `Client` programmatic API.
 *
 * `serve` is exercised through its `invoke` helper (direct, in-process)
 * for the resume-from-ledger case, and through `fetch`-shaped Request /
 * Response for the trigger / cancel flow so we prove the HTTP surface
 * also works.
 */

import { describe, expect, it, vi } from "vitest";

import { openLedger } from "@actantdb/core";

import { Client } from "./client.js";
import { serve } from "./serve.js";

describe("serve()", () => {
  it("resumes a run mid-flight by reading prior step results from the ledger", async () => {
    const ledger = openLedger({ project: "wf-resume", inMemory: true });
    const callCount = { fetch: 0, charge: 0 };

    const handler = serve(
      async (ctx) => {
        const order = await ctx.run("fetch-order", () => {
          callCount.fetch += 1;
          return { id: "ord_1", total: 99 };
        });
        const charged = await ctx.run("charge", () => {
          callCount.charge += 1;
          return { id: "ch_1", order: order.id };
        });
        return charged;
      },
      { ledger, autoResume: false },
    );

    // First call: both steps run.
    const a = await handler.invoke({ runId: "wfr_1", body: {} });
    expect(a.status).toBe("completed");
    expect(callCount).toEqual({ fetch: 1, charge: 1 });

    // Second call with same runId: both steps skip.
    const b = await handler.invoke({ runId: "wfr_1", body: {} });
    expect(b.status).toBe("completed");
    expect(callCount).toEqual({ fetch: 1, charge: 1 });
  });

  it("returns 202 with a runId on suspension", async () => {
    const ledger = openLedger({ project: "wf-202", inMemory: true });
    const handler = serve(
      async (ctx) => {
        ctx.sleep("wait", "1h");
        return "never";
      },
      { ledger, autoResume: false },
    );

    const req = new Request("https://x/wf", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ workflowRunId: "wfr_susp" }),
    });
    const res = await handler(req);
    expect(res.status).toBe(202);
    const json = (await res.json()) as { runId: string; status: string };
    expect(json.runId).toBe("wfr_susp");
    expect(json.status).toBe("suspended");
  });
});

describe("Client (local-mode against a shared ledger)", () => {
  it("trigger starts a new run via HTTP", async () => {
    const ledger = openLedger({ project: "wf-client-trigger", inMemory: true });
    const handler = serve(
      async (ctx) => {
        await ctx.run("noop", () => ctx.payload);
        return "ok";
      },
      { ledger, autoResume: false },
    );

    // Use the handler itself as the "fetch" implementation — same shape.
    const fetchImpl: typeof globalThis.fetch = async (input, init) => {
      const req = new Request(typeof input === "string" ? input : (input as Request).url, init);
      return handler(req);
    };
    const client = new Client({
      baseUrl: "https://x/wf",
      fetch: fetchImpl,
    });
    const { workflowRunId } = await client.trigger({
      body: { hello: "world" },
    });
    expect(workflowRunId).toMatch(/^wfr_/);

    // Ledger should have an agent_run_started + tool_call_completed + agent_run_finished for that run.
    const events = ledger.query({ runId: workflowRunId });
    expect(events.map((e) => e.kind)).toContain("agent_run_started");
    expect(events.map((e) => e.kind)).toContain("agent_run_finished");
  });

  it("cancel aborts the run on the next step", async () => {
    const ledger = openLedger({ project: "wf-cancel", inMemory: true });
    const stepAfterCancel = vi.fn().mockResolvedValue("nope");

    const handler = serve(
      async (ctx) => {
        await ctx.run("first", () => "ok");
        // Outside cancellation lands between invocations — simulate by
        // calling client.cancel between two invocations below.
        await ctx.run("second", stepAfterCancel);
        return "done";
      },
      { ledger, autoResume: false },
    );

    // First invocation: completes both steps (no cancel yet).
    const r0 = await handler.invoke({ runId: "wfr_cx", body: {} });
    expect(r0.status).toBe("completed");
    expect(stepAfterCancel).toHaveBeenCalledTimes(1);

    // Now cancel and try to invoke fresh — the cancel marker should make
    // any subsequent step throw run_cancelled.
    const client = new Client({ ledger });
    await client.cancel({ workflowRunId: "wfr_cx2" });

    const handler2 = serve(
      async (ctx) => {
        await ctx.run("only", () => "ok");
        return "done";
      },
      { ledger, autoResume: false },
    );
    const r1 = await handler2.invoke({ runId: "wfr_cx2", body: {} });
    expect(r1.status).toBe("cancelled");
  });
});
