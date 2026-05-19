#!/usr/bin/env node
// Third public example: a hand-rolled CLI agent.
//
// No framework. Just a loop and a tool dictionary. Demonstrates that the
// wrapper works on a bare agent — useful for tiny CLIs, codemods, or any
// scripted agent that doesn't justify Mastra/LangGraph.

import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { buildContextManifest } from "@actantdb/core";
import { withActant } from "@actantdb/mastra";
import { demoPolicy } from "@actantdb/policy";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "demo-cli";
mkdirSync(STORE_DIR, { recursive: true });

// The minimal agent shape: tools is a record of `{ execute(args) }`. Nothing
// else. This is what every framework boils down to.
const agent = {
  name: "cli-agent",
  tools: {
    "file.write": {
      id: "file.write",
      description: "Write a file",
      execute: async (args) => ({ written: true, bytes: (args.contents ?? "").length }),
    },
    "shell.run": {
      id: "shell.run",
      description: "Run a shell command",
      execute: async (args) => ({ exit: 0, stdout: `(simulated) ${args.command}` }),
    },
  },
};

const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  policy: demoPolicy,
  autoApprove: true,
});

// Plan: a simple "fix tests" loop. Read file, edit, run tests.
const plan = [
  { tool: "file.write", args: { path: "src/math.py", contents: "def add(a,b): return a+b\n" } },
  { tool: "shell.run", args: { command: "pytest -q" } },
  // This last one is a destructive cleanup — Guard will constrain it.
  { tool: "shell.run", args: { command: "rm -rf build dist" } },
];

const ctx = wrapped.startRun({ meta: { source: "cli-demo" } });
ctx.recordUserMessage("Fix tests, then clean up build artifacts.");
ctx.recordContextBuild(
  buildContextManifest([
    {
      id: "doc_cli_plan",
      kind: "document",
      source: "internal://plan",
      sensitivity: "public",
      label: "plan",
      content: JSON.stringify(plan),
    },
  ]),
);
ctx.recordModelCall({
  model: "noop:plan",
  role: "planner",
  prompt_hash: "demo",
  summary: `planned ${plan.length} tool calls`,
});

for (const step of plan) {
  await agent.tools[step.tool].execute(step.args);
}

ctx.finish({ ok: true });
wrapped.actant.close();

console.error(`✅ Recorded CLI demo for project=${PROJECT}`);
console.error(`\nOpen Studio:`);
console.error(`  ACTANTDB_STORE_DIR=${STORE_DIR} \\\n  npx actantdb studio --project ${PROJECT}`);
