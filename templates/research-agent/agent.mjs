#!/usr/bin/env node
// {{project_name}} — research-agent template.
//
// A single-step research agent: one tool (web.search) + one tool (web.fetch)
// + a synthesis step. Wrapped via @actantdb/mastra so every tool call goes
// through Guard + lands as a typed ledger event.

import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";
import { withActant } from "@actantdb/mastra";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "{{project_name}}";

const agent = {
  name: "researcher",
  tools: {
    "web.search": {
      id: "web.search",
      description: "Search the web",
      execute: async ({ query }) => ({
        results: [
          { url: "https://example.com/a", title: `Result A for ${query}` },
          { url: "https://example.com/b", title: `Result B for ${query}` },
        ],
      }),
    },
    "web.fetch": {
      id: "web.fetch",
      description: "Fetch a URL",
      execute: async ({ url }) => ({ url, body: `(stub body for ${url})` }),
    },
  },
  generate: async ({ topic }) => {
    const search = await agent.tools["web.search"].execute({ query: topic });
    const [first] = search.results;
    const fetched = await agent.tools["web.fetch"].execute({ url: first.url });
    return { topic, source: first.url, summary: fetched.body.slice(0, 200) };
  },
};

const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  autoApprove: true,
});

const r = await wrapped.run({
  message: "research the meaning of agent-shaped systems",
  input: { topic: "agent-shaped systems" },
});
console.log("result:", JSON.stringify(r.result, null, 2));
console.log(`\nStudio: npx actantdb studio --project ${PROJECT} --store-dir ${STORE_DIR} --port {{studio_port}}`);
