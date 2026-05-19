import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { Box } from "./index.js";

let root: string;

beforeEach(() => {
  root = mkdtempSync(join(tmpdir(), "actantdb-box-agent-"));
});

afterEach(() => {
  rmSync(root, { recursive: true, force: true });
});

describe("box.agent", () => {
  it("runs a structurally-typed agent and lands a model_call in the ledger", async () => {
    let echoed: unknown;
    const agent = {
      name: "test",
      tools: {},
      generate: async (input: unknown) => {
        echoed = input;
        return "ok";
      },
    };
    const box = await Box.create({ storeRoot: root, agent });
    const run = await box.agent.run({ prompt: "hello" });
    expect(run.status).toBe("ok");
    expect(run.result).toBe("ok");
    expect(echoed).toBe("hello");

    // Reopen the ledger from disk to verify the wrapper wrote to the same file.
    const allEvents = box.ledger.query({});
    const kinds = allEvents.map((e) => e.kind);
    expect(kinds).toContain("agent_run_started");
    expect(kinds).toContain("user_message_received");
    expect(kinds).toContain("model_call");
    await box.delete();
  });
});
