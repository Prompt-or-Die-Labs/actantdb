import { describe, expect, it } from "vitest";

import { ActantClient } from "./index.js";

class FakeResponse {
  constructor(
    public ok: boolean,
    public status: number,
    private body: unknown,
  ) {}
  async json(): Promise<unknown> {
    return this.body;
  }
  async text(): Promise<string> {
    return JSON.stringify(this.body);
  }
}

function makeFetcher(handler: (url: string, init?: RequestInit) => unknown): typeof fetch {
  const f = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const url = typeof input === "string" ? input : input.toString();
    const body = handler(url, init);
    return new FakeResponse(true, 200, body) as unknown as Response;
  };
  return f as typeof fetch;
}

describe("ActantClient", () => {
  it("posts a create_session command and returns the session id", async () => {
    let captured: { url: string; init?: RequestInit } | null = null;
    const fetcher = makeFetcher((url, init) => {
      captured = { url, ...(init !== undefined ? { init } : {}) };
      return {
        command_id: "cmd_1",
        event_id: "evt_1",
        result: { session_id: "sess_1" },
      };
    });
    const c = new ActantClient({ baseUrl: "http://x:4555", fetch: fetcher });
    const r = await c.createSession({ workspaceId: "ws_1", actorId: "act_1" });
    expect(r.sessionId).toBe("sess_1");
    expect(captured!.url).toContain("/v1/command");
    const body = JSON.parse(String(captured!.init!.body));
    expect(body.command_type).toBe("create_session");
    expect(body.workspace_id).toBe("ws_1");
  });

  it("attaches bearer token when configured", async () => {
    let auth: string | undefined;
    const fetcher = makeFetcher((_url, init) => {
      const headers = (init?.headers ?? {}) as Record<string, string>;
      auth = headers["authorization"];
      return { command_id: "x", result: {} };
    });
    const c = new ActantClient({ baseUrl: "http://x:4555", fetch: fetcher, token: "tok_abc" });
    await c.command({
      workspaceId: "ws_1",
      actorId: "act_1",
      commandType: "noop",
      input: {},
    });
    expect(auth).toBe("Bearer tok_abc");
  });

  it("fetches events with session_id", async () => {
    let captured = "";
    const fetcher = makeFetcher((url) => {
      captured = url;
      return { events: [] };
    });
    const c = new ActantClient({ baseUrl: "http://x:4555", fetch: fetcher });
    await c.events({ sessionId: "sess_42" });
    expect(captured).toContain("session_id=sess_42");
  });
});
