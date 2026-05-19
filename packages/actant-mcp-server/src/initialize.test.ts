import { describe, expect, it } from "vitest";

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { Ledger } from "@actantdb/core";

import { buildServer } from "./index.js";

describe("MCP initialize handshake", () => {
  it("responds to initialize with the server name and version", async () => {
    const ledger = new Ledger({ project: "ws-init", inMemory: true });
    const { server } = buildServer({ ledger, name: "actantdb-mcp-test" });

    const [serverTransport, clientTransport] = InMemoryTransport.createLinkedPair();
    await server.connect(serverTransport);

    const client = new Client({ name: "test-client", version: "0.0.0" });
    await client.connect(clientTransport);

    const tools = await client.listTools();
    const names = tools.tools.map((t) => t.name).sort();
    expect(names).toContain("list_runs");
    expect(names).toContain("get_event");
    expect(names).toContain("query_predicate");
    expect(names).toContain("replay");
    expect(names).toContain("list_pending_approvals");
    expect(names).toContain("decide_approval");
    expect(names).toContain("get_workspace_summary");

    await client.close();
    await server.close();
    ledger.close();
  });
});
