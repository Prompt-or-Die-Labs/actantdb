#!/usr/bin/env node
/**
 * `npm create actantdb@latest my-app`
 * `npx create-actantdb my-app --template coding-agent --framework mastra`
 *
 * Interactive by default; pass `--no-interactive` (or `--yes`) to take all
 * choices from argv. Missing required flags in non-interactive mode fail
 * loudly.
 */

import { existsSync, readdirSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";

import kleur from "kleur";
import prompts from "prompts";

import { renderTemplate, writeRendered } from "./render.js";
import {
  FRAMEWORKS,
  TEMPLATES,
  getTemplate,
  type FrameworkChoice,
  type LanguageChoice,
} from "./templates.js";

export { TEMPLATES, FRAMEWORKS, getTemplate } from "./templates.js";
export { renderTemplate, writeRendered } from "./render.js";

const PACKAGE_VERSION = "0.0.13";
const DEFAULT_STUDIO_PORT = 4173;

interface Args {
  positional: string[];
  template?: string;
  framework?: FrameworkChoice;
  language?: LanguageChoice;
  studioPort?: number;
  interactive: boolean;
  yes: boolean;
  force: boolean;
  help: boolean;
  version: boolean;
}

const HELP = `create-actantdb — scaffold a new ActantDB project.

Usage:
  npm create actantdb@latest <project-name>
  npx create-actantdb <project-name> [options]

Options:
  --template <id>     ${TEMPLATES.map((t) => t.id).join(" | ")}
  --framework <id>    ${FRAMEWORKS.map((f) => f.id).join(" | ")}
  --language <ts|js>  default: ts
  --port <n>          Studio port for the scaffolded project (default ${DEFAULT_STUDIO_PORT})
  --no-interactive    Skip prompts; require every choice on argv.
  --yes               Alias for --no-interactive.
  --force             Allow scaffolding into a non-empty directory.
  --help, -h          Show this message.
  --version           Print create-actantdb version.
`;

export function parseArgs(argv: string[]): Args {
  const out: Args = {
    positional: [],
    interactive: true,
    yes: false,
    force: false,
    help: false,
    version: false,
  };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]!;
    if (a === "--help" || a === "-h") out.help = true;
    else if (a === "--version") out.version = true;
    else if (a === "--no-interactive") out.interactive = false;
    else if (a === "--yes" || a === "-y") {
      out.yes = true;
      out.interactive = false;
    } else if (a === "--force") out.force = true;
    else if (a === "--template") out.template = argv[++i];
    else if (a === "--framework") out.framework = argv[++i] as FrameworkChoice;
    else if (a === "--language") out.language = argv[++i] as LanguageChoice;
    else if (a === "--port") {
      const v = argv[++i];
      if (v) out.studioPort = Number(v);
    } else if (a.startsWith("--template=")) out.template = a.slice("--template=".length);
    else if (a.startsWith("--framework="))
      out.framework = a.slice("--framework=".length) as FrameworkChoice;
    else if (a.startsWith("--language="))
      out.language = a.slice("--language=".length) as LanguageChoice;
    else if (a.startsWith("--port=")) out.studioPort = Number(a.slice("--port=".length));
    else if (!a.startsWith("-")) out.positional.push(a);
  }
  return out;
}

export interface ScaffoldChoices {
  projectName: string;
  template: string;
  framework: FrameworkChoice;
  language: LanguageChoice;
  studioPort: number;
}

export interface ScaffoldResult {
  targetDir: string;
  filesWritten: string[];
  choices: ScaffoldChoices;
}

/**
 * Headless scaffold. Used by the CLI and the test harness alike.
 */
export function scaffold(
  targetDir: string,
  choices: ScaffoldChoices,
  opts: { force?: boolean; version?: string } = {},
): ScaffoldResult {
  ensureWritableDir(targetDir, opts.force ?? false);
  const files = renderTemplate({
    projectName: choices.projectName,
    template: choices.template,
    framework: choices.framework,
    language: choices.language,
    studioPort: choices.studioPort,
    actantdbVersion: opts.version ?? `^${PACKAGE_VERSION}`,
  });
  writeRendered(targetDir, files);
  return { targetDir, filesWritten: files.map((f) => f.path), choices };
}

function ensureWritableDir(dir: string, force: boolean): void {
  if (!existsSync(dir)) {
    mkdirSync(dir, { recursive: true });
    return;
  }
  const entries = readdirSync(dir).filter((e) => !e.startsWith("."));
  if (entries.length > 0 && !force) {
    throw new Error(
      `target directory is not empty: ${dir}\n` +
        `pass --force to scaffold into it anyway (existing files will be overwritten).`,
    );
  }
}

async function interactiveFlow(args: Args): Promise<ScaffoldChoices> {
  const name =
    args.positional[0] ??
    (
      await prompts({
        type: "text",
        name: "name",
        message: "Project name?",
        initial: "my-actantdb-app",
        validate: (s: string) => (validProjectName(s) ? true : "use lowercase letters, digits, -"),
      })
    ).name;
  if (!name) throw new Error("project name is required");

  const tplChoice =
    args.template ??
    (
      await prompts({
        type: "select",
        name: "template",
        message: "Template?",
        choices: TEMPLATES.map((t) => ({
          title: `${t.title}  ${kleur.gray(`— ${t.description}`)}`,
          value: t.id,
        })),
        initial: 1,
      })
    ).template;

  const template = getTemplate(tplChoice);
  if (!template) throw new Error(`unknown template: ${tplChoice}`);

  const framework =
    args.framework ??
    (
      await prompts({
        type: "select",
        name: "framework",
        message: "Agent framework?",
        choices: FRAMEWORKS.map((f) => ({ title: f.title, value: f.id })),
        initial: FRAMEWORKS.findIndex((f) => f.id === template.defaultFramework),
      })
    ).framework;

  const language =
    args.language ??
    (
      await prompts({
        type: "select",
        name: "language",
        message: "Language?",
        choices: [
          { title: "TypeScript", value: "ts" },
          { title: "JavaScript", value: "js" },
        ],
        initial: 0,
      })
    ).language;

  return {
    projectName: name,
    template: template.id,
    framework: framework as FrameworkChoice,
    language: (language as LanguageChoice) ?? "ts",
    studioPort: args.studioPort ?? DEFAULT_STUDIO_PORT,
  };
}

function nonInteractiveFlow(args: Args): ScaffoldChoices {
  const name = args.positional[0];
  if (!name) throw new Error("project name is required in --no-interactive mode");
  if (!validProjectName(name)) {
    throw new Error(`invalid project name: "${name}" (use lowercase letters, digits, -)`);
  }
  const template = args.template ?? "minimal";
  if (!getTemplate(template)) throw new Error(`unknown template: ${template}`);
  const framework = args.framework ?? getTemplate(template)!.defaultFramework;
  const language: LanguageChoice = args.language ?? "ts";
  return {
    projectName: name,
    template,
    framework,
    language,
    studioPort: args.studioPort ?? DEFAULT_STUDIO_PORT,
  };
}

function validProjectName(name: string): boolean {
  return /^[a-z0-9][a-z0-9-]*$/.test(name);
}

export async function main(argv: string[] = process.argv.slice(2)): Promise<number> {
  const args = parseArgs(argv);
  if (args.help) {
    process.stdout.write(HELP);
    return 0;
  }
  if (args.version) {
    process.stdout.write(`create-actantdb ${PACKAGE_VERSION}\n`);
    return 0;
  }

  let choices: ScaffoldChoices;
  try {
    choices = args.interactive ? await interactiveFlow(args) : nonInteractiveFlow(args);
  } catch (err) {
    process.stderr.write(kleur.red(`error: ${(err as Error).message}\n`));
    return 1;
  }

  const targetDir = resolve(process.cwd(), choices.projectName);
  try {
    const result = scaffold(targetDir, choices, { force: args.force });
    process.stdout.write(
      [
        "",
        kleur.green("✔ scaffolded:") + ` ${kleur.bold(targetDir)}`,
        "",
        kleur.gray("Next steps:"),
        `  cd ${choices.projectName}`,
        `  npm install`,
        `  npm start`,
        `  npm run studio`,
        "",
        kleur.gray("Files written:"),
        ...result.filesWritten.map((f) => `  ${f}`),
        "",
      ].join("\n"),
    );
    return 0;
  } catch (err) {
    process.stderr.write(kleur.red(`error: ${(err as Error).message}\n`));
    return 1;
  }
}

// Detect direct execution.
const invokedAsBin =
  typeof process !== "undefined" &&
  Array.isArray(process.argv) &&
  process.argv[1] !== undefined &&
  /create-actantdb$|create-actantdb[\\/]dist[\\/]index\.js$/.test(process.argv[1]);

if (invokedAsBin || process.env["CREATE_ACTANTDB_FORCE_RUN"] === "1") {
  main().then(
    (code) => process.exit(code),
    (err) => {
      // eslint-disable-next-line no-console
      console.error(err);
      process.exit(1);
    },
  );
}
