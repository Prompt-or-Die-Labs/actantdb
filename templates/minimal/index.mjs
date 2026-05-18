#!/usr/bin/env node
// {{project_name}} — minimal ActantDB project.
//
// Records a single no-op run into ./.actantdb so you can verify the install
// path and open the run in Studio.

import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { withActant } from "@actantdb/mastra";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "{{project_name}}";

mkdirSync(STORE_DIR, { recursive: true });

const agent = {
  name: "{{project_name}}-agent",
  tools: {},
  generate: async () => "noop",
};

const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  autoApprove: true,
});

const ctx = wrapped.startRun({ meta: { source: "minimal-template" } });
ctx.recordUserMessage("Hello from the minimal template.");
ctx.finish({ ok: true });
wrapped.actant.close();

console.error(`OK — recorded a no-op run for project=${PROJECT}`);
console.error(`Studio: npx actantdb studio --project ${PROJECT} --store-dir ${STORE_DIR} --port {{studio_port}}`);
