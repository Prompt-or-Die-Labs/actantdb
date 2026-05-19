import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { api } from "./api.js";

beforeEach(() => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const payload = { url, method: init?.method ?? "GET", body: init?.body ?? null };
      return new Response(JSON.stringify(payload), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }),
  );
});

afterEach(() => vi.unstubAllGlobals());

describe("api client", () => {
  it("info() GETs /api/info", async () => {
    const r = (await api.info()) as unknown as { url: string; method: string };
    expect(r.url).toBe("/api/info");
    expect(r.method).toBe("GET");
  });

  it("events(runId) URL-encodes the run id", async () => {
    const r = (await api.events("run/with slash")) as unknown as { url: string };
    expect(r.url).toBe("/api/events?run=run%2Fwith%20slash");
  });

  it("decide() POSTs JSON with toolCallId + decision", async () => {
    const r = (await api.decide("tc_99", {
      decision: "deny",
      approver: "studio",
      reason: "test",
    })) as unknown as { url: string; method: string; body: string };
    expect(r.url).toBe("/api/approvals/decide");
    expect(r.method).toBe("POST");
    const parsed = JSON.parse(r.body);
    expect(parsed.toolCallId).toBe("tc_99");
    expect(parsed.decision.decision).toBe("deny");
  });

  it("replay() POSTs JSON with body", async () => {
    const r = (await api.replay({
      eventId: "evt_1",
      overrides: { without_memory: ["mem_x"] },
      useStrictPolicy: true,
      mode: "model",
    })) as unknown as { url: string; method: string; body: string };
    expect(r.url).toBe("/api/replay");
    expect(r.method).toBe("POST");
    const parsed = JSON.parse(r.body);
    expect(parsed.eventId).toBe("evt_1");
    expect(parsed.useStrictPolicy).toBe(true);
    expect(parsed.mode).toBe("model");
  });
});
