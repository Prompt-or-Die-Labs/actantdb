/**
 * Integration test: starts a real `actantdb-server` subprocess and exercises
 * the SDK against it. Skipped automatically if the binary isn't built yet
 * (run `cargo build -p actant-server` first).
 */

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { ActantClient } from "./index.js";

const here = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(here, "..", "..", "..");
const SERVER_BIN = join(REPO_ROOT, "target", "debug", "actantdb-server");

let proc: ReturnType<typeof spawn> | undefined;
let baseUrl = "http://127.0.0.1:0";

async function pickPort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const net = require("node:net");
    const srv = net.createServer();
    srv.listen(0, () => {
      const a = srv.address();
      const p = typeof a === "object" && a ? a.port : 0;
      srv.close(() => resolve(p));
    });
    srv.on("error", reject);
  });
}

async function waitForServer(url: string, timeoutMs = 10_000): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const r = await fetch(`${url}/v1/healthz`);
      if (r.ok) return;
    } catch {
      // not yet
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error(`server at ${url} did not start within ${timeoutMs}ms`);
}

describe.skipIf(!existsSync(SERVER_BIN))("@actantdb/sdk against real server", () => {
  beforeAll(async () => {
    const port = await pickPort();
    baseUrl = `http://127.0.0.1:${port}`;
    proc = spawn(SERVER_BIN, ["--bind", `127.0.0.1:${port}`], {
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, RUST_LOG: "warn" },
    });
    await waitForServer(baseUrl);
  }, 20_000);

  afterAll(() => {
    proc?.kill("SIGTERM");
  });

  it("creates a session and posts a tool call through the SDK", async () => {
    const c = new ActantClient({ baseUrl });
    const h = await c.healthz();
    expect(h.status).toBe("ok");

    const { sessionId } = await c.createSession({
      workspaceId: "ws_default",
      actorId: "act_system",
    });
    expect(sessionId).toMatch(/^sess_/);

    await c.appendUserMessage({
      workspaceId: "ws_default",
      actorId: "act_system",
      sessionId,
      text: "Clean up the test artifacts.",
    });

    const req = await c.requestToolCall({
      workspaceId: "ws_default",
      actorId: "act_system",
      sessionId,
      toolName: "shell.run",
      arguments: { command: "rm -rf build dist" },
    });
    expect(req.status).toBe("pending_approval");

    await c.approveToolCall({
      workspaceId: "ws_default",
      actorId: "act_system",
      toolCallId: req.toolCallId,
      scope: "once",
    });

    const { events } = await c.events({ sessionId });
    const kinds = events.map((e) => e.event_type);
    expect(kinds).toContain("session_created");
    expect(kinds).toContain("user_message_received");
    expect(kinds).toContain("tool_call_requested");
    expect(kinds).toContain("tool_call_approved");
  });

  it("subscribeIter yields a broadcast on command dispatch", async () => {
    const c = new ActantClient({ baseUrl });
    const ctrl = new AbortController();
    const iter = c.subscribeIter({
      workspaceId: "ws_default",
      kind: "events",
      signal: ctrl.signal,
    });
    // Give the subscriber a moment to register with the hub.
    await new Promise((r) => setTimeout(r, 100));
    // Dispatch a command — should trigger a broadcast.
    await c.createSession({ workspaceId: "ws_default", actorId: "act_system" });
    // Race the iterator against a 2s timeout.
    const next = (iter as AsyncIterable<unknown>)[Symbol.asyncIterator]().next();
    const winner = await Promise.race([
      next,
      new Promise((resolve) => setTimeout(() => resolve(null), 2000)),
    ]);
    ctrl.abort();
    expect(winner).not.toBeNull();
    const msg = (winner as { value: { topic: { kind: string } } }).value;
    expect(msg.topic.kind).toBe("events");
  });
});
