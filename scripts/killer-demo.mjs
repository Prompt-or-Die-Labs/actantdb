#!/usr/bin/env node
// Local rehearsal of the killer demo (see wedge/killer-demo.md).
//
// This is not the customer-facing demo repo — it's the rehearsal that proves
// the wedge produces the storyboard locally. Run with:
//
//   node scripts/killer-demo.mjs
//
// Then in another terminal:
//
//   ACTANTDB_PROJECT=demo-test-cleanup ACTANTDB_STORE_DIR=./tmp-demo-store \
//     node packages/actant-studio/dist/cli.js studio
//
// and open http://127.0.0.1:4555.

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { buildContextManifest } from "@actantdb/core";
import { withActant } from "@actantdb/mastra";
import { demoPolicy } from "@actantdb/policy";

const PROJECT = "demo-test-cleanup";
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? mkdtempSync(join(tmpdir(), "actant-demo-"));

console.error(`Killer demo using store: ${STORE_DIR}`);

// A stock-shaped Mastra agent. Two tools: shell.run + file.write.
const calls = [];
const agent = {
  name: "test-cleanup-agent",
  tools: {
    "shell.run": {
      id: "shell.run",
      description: "Run a shell command",
      execute: async (args) => {
        // Stock tool: it receives whatever Guard sealed, no special hook.
        calls.push({ tool: "shell.run", args });
        return { exit: 0, stdout: `Removed 14 files. (${args.command})` };
      },
    },
    "file.write": {
      id: "file.write",
      description: "Write a file",
      execute: async (args) => {
        calls.push({ tool: "file.write", args });
        return { written: true };
      },
    },
  },
  generate: async () => "agent finished",
};

const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  policy: demoPolicy,
  autoApprove: true,
});

const ctx = wrapped.startRun({ meta: { source: "killer-demo" } });

// The user asks the agent to clean up.
ctx.recordUserMessage("Clean up the test artifacts.");

// The agent's memory contains a stale fact.
const manifest = buildContextManifest([
  {
    id: "mem_42_dist",
    kind: "memory",
    source: "internal://memory/42",
    sensitivity: "low",
    label: 'memory: "build artifacts live under /build and /dist"',
    content: "This project's build artifacts live under both /build and /dist.",
    flags: ["stale"],
  },
  {
    id: "mem_recent_pytest",
    kind: "memory",
    source: "internal://memory/recent",
    sensitivity: "low",
    label: "memory: pytest just ran, exit 1",
    content: "Last pytest run failed; build artifacts are stale.",
  },
  {
    id: "doc_readme",
    kind: "document",
    source: "file://README.md",
    sensitivity: "public",
    label: "project README",
    content: "Project: actant-demo-test-cleanup. /dist contains release artifacts.",
  },
]);
ctx.recordContextBuild(manifest);

// The agent's planner uses the stale memory and proposes the dangerous command.
ctx.recordModelCall({
  model: "anthropic:claude-sonnet-4-6",
  role: "planner",
  prompt_hash: "demo",
  summary: 'planner: shell.run "rm -rf build dist"',
});

// Trigger Guard via the wrapped tool.
await agent.tools["shell.run"].execute({ command: "rm -rf build dist" });

ctx.finish({ ok: true });

console.error(`\n✅ Recorded killer-demo run for project=${PROJECT}`);
console.error(`Underlying tool actually called with: ${JSON.stringify(calls[0]?.args)}`);
console.error("\nOpen Studio:");
console.error(
  `  ACTANTDB_PROJECT=${PROJECT} ACTANTDB_STORE_DIR=${STORE_DIR} \\\n  node packages/actant-studio/dist/cli.js studio`,
);
console.error("\nThen in the Studio UI: click the model_call row → 'Replay from here'.");
console.error("Keep 'stricter policy' and 'exclude mem_42_dist' checked → Run replay.");

// Don't auto-clean — Studio needs the store. The caller can rm -rf after.
process.exit(0);
