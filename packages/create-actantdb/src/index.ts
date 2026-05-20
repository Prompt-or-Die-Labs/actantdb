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
    i = applyArg(argv, i, out);
  }
  return out;
}

type BooleanArg = (out: Args) => void;
type ValueArg = (out: Args, value: string | undefined) => void;

const BOOLEAN_ARGS: Record<string, BooleanArg> = {
  "--help": (out) => {
    out.help = true;
  },
  "-h": (out) => {
    out.help = true;
  },
  "--version": (out) => {
    out.version = true;
  },
  "--no-interactive": (out) => {
    out.interactive = false;
  },
  "--yes": markYes,
  "-y": markYes,
  "--force": (out) => {
    out.force = true;
  },
};

const VALUE_ARGS: Record<string, ValueArg> = {
  "--template": (out, value) => {
    if (value !== undefined) out.template = value;
  },
  "--framework": (out, value) => {
    if (value !== undefined) out.framework = value as FrameworkChoice;
  },
  "--language": (out, value) => {
    if (value !== undefined) out.language = value as LanguageChoice;
  },
  "--port": (out, value) => {
    if (value) out.studioPort = Number(value);
  },
};

function applyArg(argv: string[], index: number, out: Args): number {
  const arg = argv[index]!;
  const booleanArg = BOOLEAN_ARGS[arg];
  if (booleanArg) {
    booleanArg(out);
    return index;
  }

  const valueArg = VALUE_ARGS[arg];
  if (valueArg) {
    valueArg(out, argv[index + 1]);
    return index + 1;
  }

  const assigned = splitAssignedArg(arg);
  if (assigned) {
    assigned.parser(out, assigned.value);
  } else if (!arg.startsWith("-")) {
    out.positional.push(arg);
  }
  return index;
}

function splitAssignedArg(arg: string): { parser: ValueArg; value: string } | undefined {
  const eq = arg.indexOf("=");
  if (eq < 0) return undefined;
  const parser = VALUE_ARGS[arg.slice(0, eq)];
  if (!parser) return undefined;
  return { parser, value: arg.slice(eq + 1) };
}

function markYes(out: Args): void {
  out.yes = true;
  out.interactive = false;
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
