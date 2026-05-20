/**
 * Inline template renderer.
 *
 * The scaffolder ships every template's file tree as a static map of
 * file paths -> contents, with `{{project_name}}` / `{{framework}}` /
 * `{{language}}` tokens replaced at write time.
 *
 * Why inline instead of reading from the monorepo's `/templates/`?
 *
 *   - `create-actantdb` is published as a standalone npm package, so we
 *     can't assume the monorepo is on the user's disk.
 *   - The inline templates cover the documented surface (minimal +
 *     coding-agent). Richer templates fetched from `@actantdb/templates`
 *     are wired up in a later release.
 */

import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";

import type { FrameworkChoice, LanguageChoice, RuntimeChoice } from "./templates.js";

export interface RenderContext {
  projectName: string;
  template: string;
  framework: FrameworkChoice;
  language: LanguageChoice;
  runtime: RuntimeChoice;
  studioPort: number;
  actantdbVersion: string;
}

export interface RenderedFile {
  path: string;
  content: string;
  mode?: number;
}

export function renderTemplate(ctx: RenderContext): RenderedFile[] {
  const files: RenderedFile[] = [];
  files.push({ path: "package.json", content: packageJson(ctx) });
  files.push({ path: "README.md", content: readme(ctx) });
  files.push({ path: ".gitignore", content: gitignore() });
  const entry = ctx.language === "ts" ? "src/agent.ts" : "agent.mjs";
  files.push({ path: entry, content: agentEntry(ctx) });
  if (ctx.language === "ts") {
    files.push({ path: "tsconfig.json", content: tsconfig() });
  }
  return files;
}

export function writeRendered(targetDir: string, files: RenderedFile[]): void {
  for (const f of files) {
    const dest = join(targetDir, f.path);
    mkdirSync(dirname(dest), { recursive: true });
    writeFileSync(dest, f.content, "utf8");
  }
}

// --- file templates ---------------------------------------------------------

function packageJson(ctx: RenderContext): string {
  const isTs = ctx.language === "ts";
  const runtimeBin = ctx.runtime === "bun" ? "bun" : "node";
  const entry = isTs ? "tsx src/agent.ts" : `${runtimeBin} agent.mjs`;
  const buildScript = isTs ? "tsc -p tsconfig.json" : 'echo "no build step"';

  const deps: Record<string, string> = {
    "@actantdb/core": ctx.actantdbVersion,
    "@actantdb/mastra": ctx.actantdbVersion,
    "@actantdb/policy": ctx.actantdbVersion,
    "@actantdb/types": ctx.actantdbVersion,
  };
  if (ctx.framework === "mastra") deps["@mastra/core"] = "^1.0.0";

  const devDeps: Record<string, string> = {
    "@actantdb/studio": ctx.actantdbVersion,
  };
  if (isTs) {
    devDeps["typescript"] = "^5.4.0";
    devDeps["tsx"] = "^4.7.0";
    devDeps["@types/node"] = "^22.0.0";
  }

  const body = {
    name: ctx.projectName,
    version: "0.0.1",
    private: true,
    type: "module",
    description: `${ctx.projectName} — ActantDB ${ctx.template} project.`,
    scripts: {
      start: entry,
      build: buildScript,
      studio: `actantdb studio --project ${ctx.projectName} --store-dir ./.actantdb --port ${ctx.studioPort}`,
      doctor: "actantdb --db ./.actantdb/actant.db doctor",
    },
    dependencies: deps,
    devDependencies: devDeps,
    engines: ctx.runtime === "bun" ? { bun: ">=1.3" } : { node: ">=22.5" },
  };
  return JSON.stringify(body, null, 2) + "\n";
}

function tsconfig(): string {
  return (
    JSON.stringify(
      {
        compilerOptions: {
          target: "ES2022",
          module: "ESNext",
          moduleResolution: "Bundler",
          outDir: "dist",
          rootDir: "src",
          strict: true,
          esModuleInterop: true,
          isolatedModules: true,
          skipLibCheck: true,
        },
        include: ["src/**/*"],
      },
      null,
      2,
    ) + "\n"
  );
}

function gitignore(): string {
  return ["node_modules", "dist", ".actantdb", ".env", ".env.local"].join("\n") + "\n";
}

function agentEntry(ctx: RenderContext): string {
  const importExt = ctx.language === "ts" ? "" : ".js";
  // Even .mjs uses bare specifiers, so we don't need the .js suffix for npm pkgs.
  void importExt;
  const header = ctx.language === "ts"
    ? `// ${ctx.projectName} — ActantDB ${ctx.template} agent (${ctx.framework}).\n`
    : `// ${ctx.projectName} — ActantDB ${ctx.template} agent (${ctx.framework}).\n`;

  // For the minimal template we ship the same code as templates/minimal/index.mjs,
  // adapted to live in the scaffolded project.
  if (ctx.template === "minimal" || ctx.framework === "hand-rolled") {
    return (
      header +
      `import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { withActant } from "@actantdb/mastra";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, ".actantdb");
const PROJECT = "${ctx.projectName}";

mkdirSync(STORE_DIR, { recursive: true });

const agent = {
  name: "${ctx.projectName}-agent",
  tools: {},
  generate: async () => "noop",
};

const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  autoApprove: true,
});

const ctx = wrapped.startRun({ meta: { source: "${ctx.template}" } });
ctx.recordUserMessage("Hello from the ${ctx.template} template.");
ctx.finish({ ok: true });
wrapped.actant.close();

process.stdout.write(\`OK - recorded a no-op run for project=\${PROJECT}\\n\`);
process.stdout.write(\`Studio: npx actantdb studio --project \${PROJECT} --store-dir \${STORE_DIR} --port ${ctx.studioPort}\\n\`);
`
    );
  }

  // Coding-agent + Mastra (or other framework) — wraps a stub tool record.
  return (
    header +
    `import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { withActant } from "@actantdb/mastra";
import { demoPolicy } from "@actantdb/policy";

const here = dirname(fileURLToPath(import.meta.url));
const STORE_DIR = process.env.ACTANTDB_STORE_DIR ?? join(here, "..", ".actantdb");
const PROJECT = "${ctx.projectName}";

mkdirSync(STORE_DIR, { recursive: true });

// Replace this stub with your real ${ctx.framework} agent. The wrapper is
// framework-agnostic: it only requires a \`tools\` record and a \`generate\`
// function (or anything that calls those tools).
const agent = {
  name: "${ctx.projectName}-agent",
  tools: {
    "shell.run": {
      id: "shell.run",
      description: "Run a shell command",
      execute: async (args) => ({ exit: 0, stdout: \`(stub) \${args.command}\` }),
    },
  },
  generate: async () => "agent ran",
};

const wrapped = withActant(agent, {
  project: PROJECT,
  storeDir: STORE_DIR,
  policy: demoPolicy,
  autoApprove: true,
});

const run = wrapped.startRun({ meta: { source: "${ctx.template}" } });
run.recordUserMessage("Clean up the test artifacts.");
await agent.tools["shell.run"].execute({ command: "rm -rf build dist" });
run.finish({ ok: true });
wrapped.actant.close();

process.stdout.write(\`OK - recorded a sample run for project=\${PROJECT}\\n\`);
process.stdout.write(\`Studio: npx actantdb studio --project \${PROJECT} --store-dir \${STORE_DIR} --port ${ctx.studioPort}\\n\`);
`
  );
}

function readme(ctx: RenderContext): string {
  const entry = ctx.language === "ts" ? "src/agent.ts" : "agent.mjs";
  return `# ${ctx.projectName}

Scaffolded with \`create-actantdb\` — template **${ctx.template}**, framework **${ctx.framework}**.

## Run

\`\`\`bash
npm install
npm start            # runs ${entry}
npm run studio       # opens the timeline at http://localhost:${ctx.studioPort}
npm run doctor       # checks the embedded ledger
\`\`\`

## What's here

- \`${entry}\` — your agent, wrapped by \`withActant()\` so every tool call,
  every Guard verdict, and every approval lands in the local ledger.
- \`.actantdb/\` — the embedded SQLite ledger (gitignored).

## Next steps

1. Replace the stub \`tools\` with your real tools.
2. Replace \`demoPolicy\` with the policy your team agreed on.
3. Open Studio (\`npm run studio\`), drive a run, click any tool call,
   and try **Replay** with a memory excluded.

## Docs

- ActantDB recipes:   https://github.com/Prompt-or-Die-Labs/actantdb/tree/main/docs/recipes
- @actantdb/mastra:   https://www.npmjs.com/package/@actantdb/mastra
- @actantdb/policy:   https://www.npmjs.com/package/@actantdb/policy
`;
}
