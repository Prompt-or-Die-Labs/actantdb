import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { App } from "./App.js";

// Minimal fixture mirroring shapes the server actually returns.
function fixtureInfo() {
  return {
    project: "demo",
    dbPath: "/tmp/demo.sqlite",
    runs: [
      { runId: "run_aaaaaaaaaaaa", events: 3, startedAt: "2026-05-19T12:00:00.000Z" },
      { runId: "run_bbbbbbbbbbbb", events: 5, startedAt: "2026-05-19T12:30:00.000Z" },
    ],
  };
}

function fixtureEvents() {
  return {
    events: [
      {
        id: "evt_1",
        run_id: "run_bbbbbbbbbbbb",
        kind: "agent_run_started",
        payload: {},
        sensitivity: "low",
        chain_hash: "a".repeat(64),
        created_at: "2026-05-19T12:30:00.123Z",
      },
      {
        id: "evt_2",
        run_id: "run_bbbbbbbbbbbb",
        kind: "model_call",
        payload: { role: "planner", model: "noop", prompt_hash: "h", summary: "plan it" },
        sensitivity: "low",
        chain_hash: "b".repeat(64),
        created_at: "2026-05-19T12:30:01.000Z",
      },
      {
        id: "evt_3",
        run_id: "run_bbbbbbbbbbbb",
        kind: "approval_required",
        payload: {
          tool: "shell.run",
          tool_call_id: "tc_42",
          args: { command: "rm -rf build dist" },
          constrained_input: { command: "rm -rf build" },
          hint: "drop dist",
        },
        sensitivity: "low",
        chain_hash: "c".repeat(64),
        created_at: "2026-05-19T12:30:02.000Z",
      },
    ],
  };
}

// Track fetch calls so tests can assert against them.
interface FetchCall {
  url: string;
  init?: RequestInit;
}

let calls: FetchCall[] = [];

beforeEach(() => {
  calls = [];
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    calls.push({ url, ...(init ? { init } : {}) });
    if (url.endsWith("/api/info")) {
      return new Response(JSON.stringify(fixtureInfo()), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    if (url.includes("/api/events")) {
      return new Response(JSON.stringify(fixtureEvents()), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    if (url.endsWith("/api/approvals")) {
      return new Response(JSON.stringify({ approvals: [] }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    if (url.endsWith("/api/approvals/decide")) {
      return new Response(JSON.stringify({ approval: { status: "approved" } }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }
    if (url.endsWith("/api/replay")) {
      return new Response(
        JSON.stringify({
          replay: { events: [] },
          diff: {
            entries: [
              { kind: "model_call", diff: "changed", a: "old", b: "new" },
            ],
          },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }
    return new Response("not found", { status: 404 });
  });
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
  cleanup();
});

describe("Actant Studio App", () => {
  it("renders the topbar with project + dbPath from /api/info", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText(/demo · \/tmp\/demo\.sqlite/)).toBeInTheDocument();
    });
    expect(calls.some((c) => c.url.endsWith("/api/info"))).toBe(true);
  });

  it("lists runs and auto-selects the latest run, then fetches events for it", async () => {
    render(<App />);
    // Two runs visible
    await waitFor(() => {
      expect(screen.getByText(/run_bbbbbb…/)).toBeInTheDocument();
    });
    // Auto-selected = last run → fetches /api/events?run=run_bbbbbbbbbbbb
    await waitFor(() => {
      expect(
        calls.some((c) =>
          c.url.includes("/api/events?run=run_bbbbbbbbbbbb"),
        ),
      ).toBe(true);
    });
    // Event kinds appear in the timeline
    await waitFor(() => {
      expect(screen.getByText("model_call")).toBeInTheDocument();
      expect(screen.getByText("approval_required")).toBeInTheDocument();
    });
  });

  it("posts to /api/approvals/decide when user clicks Approve on a selected event", async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => screen.getByText("approval_required"));
    // Click the approval_required row in the timeline.
    await user.click(screen.getByText("approval_required"));
    // The detail panel surfaces an Approve button.
    const approveBtn = await screen.findByRole("button", { name: /^Approve$/ });
    await user.click(approveBtn);
    await waitFor(() => {
      const decideCall = calls.find((c) => c.url.endsWith("/api/approvals/decide"));
      expect(decideCall).toBeDefined();
      const body = JSON.parse(String(decideCall!.init!.body));
      expect(body.toolCallId).toBe("tc_42");
      expect(body.decision.decision).toBe("approve");
    });
  });

  it("posts to /api/replay with overrides + strict policy when Run replay is clicked", async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => screen.getByText("model_call"));
    await user.click(screen.getByText("model_call"));
    const runBtn = await screen.findByRole("button", { name: /Run replay/ });
    await user.click(runBtn);
    await waitFor(() => {
      const replayCall = calls.find((c) => c.url.endsWith("/api/replay"));
      expect(replayCall).toBeDefined();
      const body = JSON.parse(String(replayCall!.init!.body));
      expect(body.eventId).toBe("evt_2");
      expect(body.useStrictPolicy).toBe(true);
      expect(body.overrides.without_memory).toContain("mem_42_dist");
      expect(["recorded", "model", "policy", "memory"]).toContain(body.mode);
    });
    // The diff renders.
    await waitFor(() => {
      expect(screen.getByText("changed")).toBeInTheDocument();
    });
  });
});
