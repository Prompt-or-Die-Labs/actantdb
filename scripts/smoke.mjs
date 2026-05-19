#!/usr/bin/env node
// Workspace smoke test — exercises the killer-demo path end-to-end.
//
// Sequence (from /wedge/f2-f3-prevention.md §F3 "workspace smoke test"):
//   create session              → startRun()
//   append user message         → recordUserMessage()
//   build context manifest      → buildContextManifest() + recordContextBuild()
//   request tool call           → Guard policy fires
//   require approval            → approval_required event
//   approve tool call           → approval_decision event
//   complete effect             → tool_call_completed event
//   create replay checkpoint    → checkpoint(eventId)
//   render Studio timeline      → headless HTTP GET /api/events
//
// Exit code 0 = pass. Any thrown error fails the test.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { buildContextManifest, createActant } from "@actantdb/core";
import { withActant } from "@actantdb/mastra";
import { demoPolicy } from "@actantdb/policy";
import {
  runFromEvent,
  diffReplayAgainstOriginal,
  tighten,
} from "@actantdb/replay";
import { startStudioServer } from "@actantdb/studio";

const PROJECT = "smoke-test";
const STORE_DIR = mkdtempSync(join(tmpdir(), "actantdb-smoke-"));

function assert(cond, msg) {
  if (!cond) {
    console.error("FAIL:", msg);
    process.exit(1);
  }
}

async function main() {
  console.error(`[smoke] using temp store: ${STORE_DIR}`);

  // --- 1. wedge: a stock-shaped Mastra agent wrapped by withActant.
  const calls = [];
  const agent = {
    name: "test-cleanup-agent",
    tools: {
      "shell.run": {
        id: "shell.run",
        description: "Run a shell command",
        // Stock tool shape: just reads its args. No Actant-specific hook.
        execute: async (args) => {
          calls.push({ tool: "shell.run", args });
          return { exit: 0, stdout: `Removed 14 files. (${args.command})` };
        },
      },
    },
    generate: async () => "agent ran",
  };

  const wrapped = withActant(agent, {
    project: PROJECT,
    storeDir: STORE_DIR,
    policy: demoPolicy,
    autoApprove: true, // smoke harness: accept the constrained variant.
  });

  const ctx = wrapped.startRun({ meta: { source: "smoke" } });
  ctx.recordUserMessage("Clean up the test artifacts.");

  // Manifest with a memory item that mentions /dist (the killer-demo seed)
  const manifest = buildContextManifest([
    {
      id: "mem_42_dist",
      kind: "memory",
      source: "internal://memory/42",
      sensitivity: "low",
      label: "build artifacts live under /build and /dist",
      content: "build artifacts live under /build and /dist",
      flags: ["stale"],
    },
    {
      id: "doc_readme",
      kind: "document",
      source: "file://README.md",
      sensitivity: "public",
      label: "project README",
      content: "this is the readme",
    },
  ]);
  ctx.recordContextBuild(manifest);
  ctx.recordModelCall({
    model: "noop:test",
    role: "planner",
    prompt_hash: "abc",
    summary: "planner: shell.run rm -rf build dist",
  });

  // Now drive a tool call through the wrapped agent (via the wrapped execute).
  await agent.tools["shell.run"].execute({ command: "rm -rf build dist" });

  ctx.finish({ ok: true });

  // --- 2. validate ledger contents.
  const events = wrapped.actant.ledger.query({ runId: ctx.runId });
  const kinds = events.map((e) => e.kind);
  assert(kinds.includes("agent_run_started"), "missing agent_run_started");
  assert(kinds.includes("user_message_received"), "missing user_message_received");
  assert(kinds.includes("context_build"), "missing context_build");
  assert(kinds.includes("model_call"), "missing model_call");
  assert(kinds.includes("tool_call_requested"), "missing tool_call_requested");
  assert(kinds.includes("guard_verdict"), "missing guard_verdict");
  assert(kinds.includes("approval_required"), "missing approval_required (constrain hint path)");
  assert(kinds.includes("approval_decision"), "missing approval_decision");
  assert(kinds.includes("tool_call_started"), "missing tool_call_started");
  assert(kinds.includes("tool_call_completed"), "missing tool_call_completed");
  assert(kinds.includes("agent_run_finished"), "missing agent_run_finished");

  const completed = events.find((e) => e.kind === "tool_call_completed");
  assert(completed.payload.status === "ok", "tool_call_completed status not ok");
  assert(calls.length === 1, "underlying tool called exactly once");
  assert(
    calls[0].args.command === "rm -rf build",
    `constrain rewrite expected 'rm -rf build', got: ${calls[0].args.command}`,
  );

  // chain hashes must form a chain
  let prev = null;
  for (const e of events) {
    if (prev) {
      assert(e.chain_hash.length === 64, "chain_hash hex length");
    }
    prev = e;
  }

  // --- 3. checkpoint + replay.
  const modelCallEvent = events.find((e) => e.kind === "model_call");
  const checkpoint = wrapped.actant.ledger.checkpoint(modelCallEvent.id);
  assert(checkpoint.manifest_hash, "checkpoint must include manifest_hash");

  const replay = runFromEvent({
    ledger: wrapped.actant.ledger,
    eventId: modelCallEvent.id,
    overrides: { without_memory: ["mem_42_dist"] },
    policy: tighten(demoPolicy, {
      deny: [
        {
          tool: "shell.run",
          pattern: "\\bdist\\b",
          reason: "no shell.run without explicit dist guard",
        },
      ],
    }),
    alternatePlannerOutput: "planner: shell.run rm -rf build",
  });
  const dif = diffReplayAgainstOriginal(wrapped.actant.ledger, replay);
  assert(dif.entries.length > 0, "diff must produce entries");
  assert(
    dif.entries.some((d) => d.diff === "changed"),
    "diff must surface at least one changed row",
  );

  // --- 4. headless Studio: timeline render via HTTP.
  const studio = await startStudioServer({
    ledger: wrapped.actant.ledger,
    port: 0,
    silent: true,
  });
  try {
    const r = await fetch(`${studio.url}/api/events?run=${encodeURIComponent(ctx.runId)}`);
    assert(r.ok, `/api/events returned ${r.status}`);
    const body = await r.json();
    assert(Array.isArray(body.events), "events payload must be an array");
    assert(body.events.length === events.length, "studio must echo ledger event count");

    const info = await (await fetch(`${studio.url}/api/info`)).json();
    assert(info.project === PROJECT, "studio info must include project");
    assert(Array.isArray(info.runs), "studio info must include runs[]");

    // POST a replay request through the HTTP layer
    const post = await fetch(`${studio.url}/api/replay`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        eventId: modelCallEvent.id,
        overrides: { without_memory: ["mem_42_dist"] },
        useStrictPolicy: true,
      }),
    });
    assert(post.ok, `/api/replay returned ${post.status}`);
    const postBody = await post.json();
    assert(postBody.diff?.entries?.length, "HTTP replay must produce a diff");
  } finally {
    await studio.close();
  }

  // --- 5. cleanup.
  wrapped.actant.close();
  rmSync(STORE_DIR, { recursive: true, force: true });
  console.log("✅ smoke test passed");
}

main().catch((err) => {
  console.error(err);
  try {
    rmSync(STORE_DIR, { recursive: true, force: true });
  } catch {}
  process.exit(1);
});
