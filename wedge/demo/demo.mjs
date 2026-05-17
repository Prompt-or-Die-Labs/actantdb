#!/usr/bin/env node
// Killer-demo rehearsal. Records the storyboard from /wedge/killer-demo.md.
//
//   pnpm --filter actant-demo-test-cleanup demo
//   pnpm --filter actant-demo-test-cleanup studio   # in another terminal
//
// Or directly:
//   node wedge/demo/demo.mjs

import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { buildContextManifest } from "@actantdb/core";
import { withActant } from "@actantdb/mastra";
import { demoPolicy } from "@actantdb/policy";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "demo-test-cleanup";

mkdirSync(STORE_DIR, { recursive: true });

// A stock-shaped Mastra agent: tools is a record of { execute(args) }.
const calls = [];
const agent = {
  name: "test-cleanup-agent",
  tools: {
    "shell.run": {
      id: "shell.run",
      description: "Run a shell command",
      execute: async (args) => {
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
  generate: async () => "agent done",
};

const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  policy: demoPolicy,
  autoApprove: true, // demo accepts the constrained variant; live mode prompts the operator.
});

const ctx = wrapped.startRun({ meta: { source: "killer-demo" } });
ctx.recordUserMessage("Clean up the test artifacts.");

// Manifest with a stale memory that mentions /dist (the killer-demo seed).
ctx.recordContextBuild(
  buildContextManifest([
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
      content: "Last pytest run failed.",
    },
    {
      id: "doc_readme",
      kind: "document",
      source: "file://README.md",
      sensitivity: "public",
      label: "project README",
      content: "/dist contains release artifacts.",
    },
  ]),
);
ctx.recordModelCall({
  model: "anthropic:claude-sonnet-4-6",
  role: "planner",
  prompt_hash: "demo",
  summary: 'planner: shell.run "rm -rf build dist"',
});

await agent.tools["shell.run"].execute({ command: "rm -rf build dist" });

ctx.finish({ ok: true });
wrapped.actant.close();

console.error(`✅ Recorded killer-demo run for project=${PROJECT}`);
console.error(`Tool actually executed with: ${JSON.stringify(calls[0]?.args)}`);
console.error(`\nNext: open Studio in another terminal:`);
console.error(`  ACTANTDB_STORE_DIR=${STORE_DIR} \\\n  npx actantdb studio --project ${PROJECT}`);
console.error(`\nThen in Studio: click model_call → Replay from here → Run replay.`);
