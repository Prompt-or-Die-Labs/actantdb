#!/usr/bin/env node
// {{project_name}} — fan-out / concurrent-sessions template.
//
// Spawns N agent runs in parallel, each one a small unit of work, all
// captured to the same ledger. Use this template to test that Studio
// renders timelines for many simultaneous runs.

import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { withActant } from "@actantdb/mastra";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "{{project_name}}";
const N = Number(process.env.N ?? 20);

const agent = {
  name: "worker",
  tools: {
    work: {
      id: "work",
      execute: async ({ id }) => ({ processed: id, value: id * 2 }),
    },
  },
  generate: async ({ id }) => await agent.tools.work.execute({ id }),
};

const wrapped = withActant(agent, { project: PROJECT, storeDir: STORE_DIR, autoApprove: true });

const t0 = performance.now();
const runs = await Promise.all(
  Array.from({ length: N }, (_, i) =>
    wrapped.run({ message: `process ${i}`, input: { id: i } }),
  ),
);
const elapsed = performance.now() - t0;
console.log(`${N} concurrent runs in ${elapsed.toFixed(0)}ms (${(elapsed / N).toFixed(1)}ms/run avg)`);
console.log(`\nStudio: npx actantdb studio --project ${PROJECT} --store-dir ${STORE_DIR} --port {{studio_port}}`);
