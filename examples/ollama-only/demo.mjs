#!/usr/bin/env node

import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { buildContextManifest } from "@actantdb/core";
import { withActant } from "@actantdb/mastra";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "demo-ollama-only";
const OLLAMA_BASE_URL = process.env.OLLAMA_BASE_URL ?? "http://127.0.0.1:11434";
const OLLAMA_MODEL = process.env.OLLAMA_MODEL ?? "llama3.2:8b";
const USE_MOCK = process.env.ACTANTDB_OLLAMA_MOCK === "1";

mkdirSync(STORE_DIR, { recursive: true });

const policy = {
  label: "local-model-only",
  sensitivity_ceiling: "low",
  tools: [],
  deny: [
    {
      tool: "openai_completion",
      pattern: ".*",
      reason: "cloud completions are disabled for this project",
    },
    {
      tool: "anthropic_completion",
      pattern: ".*",
      reason: "cloud completions are disabled for this project",
    },
  ],
};

const agent = {
  name: "ollama-only-agent",
  tools: {
    openai_completion: {
      id: "openai_completion",
      description: "A cloud completion endpoint that must stay blocked",
      execute: async () => {
        throw new Error("cloud completion tool should not execute");
      },
    },
  },
};

const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  policy,
});

const prompt = "Summarize why local-only agent traces matter in one sentence.";
const ctx = wrapped.startRun({ meta: { source: "ollama-only-demo", model: OLLAMA_MODEL } });

try {
  ctx.recordUserMessage(prompt);
  ctx.recordContextBuild(
    buildContextManifest([
      {
        id: "doc_local_boundary",
        kind: "document",
        source: "internal://local-model-boundary",
        sensitivity: "low",
        label: "local model boundary",
        content: "This run must use a local Ollama model and block cloud completion tools.",
      },
    ]),
  );

  const answer = await ollamaGenerate(prompt);
  ctx.recordModelCall({
    model: `ollama:${OLLAMA_MODEL}`,
    role: "generator",
    prompt_hash: "ollama-demo",
    summary: answer.slice(0, 200),
  });

  const blocked = await agent.tools.openai_completion.execute({ prompt });
  ctx.finish({ ok: true, blocked });

  console.error(`Recorded Ollama-only demo for project=${PROJECT}`);
  console.error(`   model=${USE_MOCK ? "mock:" : "ollama:"}${OLLAMA_MODEL}`);
  console.error(`   cloud completion attempt: ${JSON.stringify(blocked)}`);
  console.error("");
  console.error("Open Studio:");
  console.error(`  ACTANTDB_STORE_DIR=${STORE_DIR} \\`);
  console.error(`  npx actantdb studio --project ${PROJECT}`);
} catch (err) {
  ctx.finish({ ok: false, error: err instanceof Error ? err.message : String(err) });
  throw err;
} finally {
  wrapped.actant.close();
}

async function ollamaGenerate(promptText) {
  if (USE_MOCK) {
    return "Local-only traces keep the model prompt, tool policy, and blocked cloud calls inspectable without leaving the machine.";
  }

  const response = await fetch(`${OLLAMA_BASE_URL}/api/generate`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      model: OLLAMA_MODEL,
      prompt: promptText,
      stream: false,
    }),
  }).catch((err) => {
    throw new Error(
      `Ollama is not reachable at ${OLLAMA_BASE_URL}; start Ollama or run ACTANTDB_OLLAMA_MOCK=1 node demo.mjs for the deterministic local mock. ${err instanceof Error ? err.message : String(err)}`,
    );
  });

  if (!response.ok) {
    throw new Error(`Ollama returned HTTP ${response.status}; run \`ollama pull ${OLLAMA_MODEL}\` if the model is missing.`);
  }

  const body = await response.json();
  if (!body || typeof body !== "object" || typeof body.response !== "string") {
    throw new Error("Ollama response did not include a string `response` field.");
  }
  return body.response;
}
