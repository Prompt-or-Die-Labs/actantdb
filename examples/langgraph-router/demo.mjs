#!/usr/bin/env node
// Second public example: a LangGraph-shaped routing agent.
//
// LangGraph models agents as state machines where nodes call tools.
// Our wrapper is duck-typed — it doesn't care about the orchestration loop,
// only that `agent.tools[name].execute(args)` exists. This demo shows the
// SAME wrapper handling a non-Mastra agent shape.

import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { buildContextManifest } from "@actantdb/core";
import { withActant } from "@actantdb/langgraph";
import { demoPolicy } from "@actantdb/policy";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "demo-langgraph-router";
mkdirSync(STORE_DIR, { recursive: true });

// A LangGraph-shaped agent: nodes call tools. We expose the tool registry
// the way LangGraph nodes would — `tools.<name>.execute(args)`.
const calls = [];
const agent = {
  name: "router-agent",
  // In LangGraph, tools are stored alongside the graph definition. Here we
  // expose them at the top level the way Mastra would, and provide a
  // `invoke` (the LangGraph convention for "step the graph") instead of
  // `generate`.
  tools: {
    "file.read": {
      id: "file.read",
      description: "Read a file",
      execute: async (args) => {
        calls.push({ tool: "file.read", args });
        return { contents: `[stub] would read ${args.path}` };
      },
    },
    "http.get": {
      id: "http.get",
      description: "Issue an HTTP GET request",
      execute: async (args) => {
        calls.push({ tool: "http.get", args });
        return { status: 200, body: `[stub] would fetch ${args.url}` };
      },
    },
    "shell.run": {
      id: "shell.run",
      description: "Run a shell command (high-risk; gated by Guard)",
      execute: async (args) => {
        calls.push({ tool: "shell.run", args });
        return { exit: 0, stdout: `(simulated) ran ${args.command}` };
      },
    },
  },
  // LangGraph nodes don't call `generate`; they invoke the graph. We expose
  // the equivalent here so withActant can capture a model_call event.
  invoke: async (input) => {
    return `routed via ${JSON.stringify(input)}`;
  },
};

// withActant wraps the agent: it monkey-patches `tools[name].execute` to go
// through Guard before invoking the real handler. The orchestration loop
// (Mastra agent.generate, LangGraph graph.invoke, anything) is untouched.
const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  policy: demoPolicy,
  autoApprove: true,
});

const ctx = wrapped.startRun({ meta: { source: "langgraph-demo" } });
ctx.recordUserMessage("Fetch the homepage of example.com and grep for 'foo'.");

ctx.recordContextBuild(
  buildContextManifest([
    {
      id: "mem_http_safe",
      kind: "memory",
      source: "internal://memory/http-safe",
      sensitivity: "low",
      label: "memory: example.com is safe to fetch",
      content: "example.com is safe to fetch without approval.",
    },
    {
      id: "doc_routing_rules",
      kind: "document",
      source: "file://routing.md",
      sensitivity: "public",
      label: "routing rules",
      content: "Fetch via http.get; never via curl through shell.run.",
    },
  ]),
);

ctx.recordModelCall({
  model: "anthropic:claude-sonnet-4-6",
  role: "router",
  prompt_hash: "demo",
  summary: "router: http.get example.com, then grep via shell.run",
});

// Step 1: a safe read.
await agent.tools["http.get"].execute({ url: "https://example.com" });

// Step 2: a destructive shell command — Guard demands approval.
await agent.tools["shell.run"].execute({ command: "rm -rf cache dist" });

ctx.finish({ ok: true });
wrapped.actant.close();

console.error(`Recorded LangGraph demo for project=${PROJECT}`);
console.error(`   ${calls.length} tool calls executed.`);
console.error(`   The shell.run was constrained: ${JSON.stringify(calls[calls.length - 1]?.args)}`);
console.error(`\nOpen Studio:`);
console.error(`  ACTANTDB_STORE_DIR=${STORE_DIR} \\\n  npx actantdb studio --project ${PROJECT}`);
