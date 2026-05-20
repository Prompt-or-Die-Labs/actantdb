#!/usr/bin/env node
// {{project_name}} — coding agent template.
//
// A Mastra-shaped agent with shell.run and file.write tools, wrapped through
// @actantdb/mastra. Mirrors the alpha demo at /wedge/demo/demo.mjs in the
// ActantDB repo.

import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { buildContextManifest } from "@actantdb/core";
import { withActant } from "@actantdb/mastra";
import { demoPolicy } from "@actantdb/policy";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "{{project_name}}";

mkdirSync(STORE_DIR, { recursive: true });

// A stock-shaped Mastra agent: tools is a record of { execute(args) }.
const calls = [];
const agent = {
  name: "{{project_name}}-agent",
  tools: {
    "shell.run": {
      id: "shell.run",
      description: "Run a shell command",
      execute: async (args) => {
        calls.push({ tool: "shell.run", args });
        return { exit: 0, stdout: `Pretended to run: ${args.command}` };
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
  autoApprove: true, // template ships in demo mode; live mode prompts the operator.
});

const ctx = wrapped.startRun({ meta: { source: "coding-agent-template" } });
ctx.recordUserMessage("Clean up the test artifacts.");

// A context manifest with one stale memory — Guard should constrain the proposal
// that this memory drives.
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
  prompt_hash: "{{project_name}}-stub",
  summary: 'planner: shell.run "rm -rf build dist"',
});

await agent.tools["shell.run"].execute({ command: "rm -rf build dist" });

ctx.finish({ ok: true });
wrapped.actant.close();

process.stdout.write(`OK - recorded a constrained run for project=${PROJECT}\n`);
process.stdout.write(`Tool actually executed with: ${JSON.stringify(calls[0]?.args)}\n`);
process.stdout.write(`Studio: npx actantdb studio --project ${PROJECT} --store-dir ${STORE_DIR} --port {{studio_port}}\n`);
