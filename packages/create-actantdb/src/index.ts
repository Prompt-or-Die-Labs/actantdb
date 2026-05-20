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

const PACKAGE_VERSION = "0.0.15";
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

class UserFacingError extends Error {
  readonly fix?: string;
  readonly detail?: string;

  constructor(message: string, opts: { fix?: string; detail?: string } = {}) {
    super(message);
    this.name = "UserFacingError";
    this.fix = opts.fix;
    this.detail = opts.detail;
  }
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
    const value = argv[index + 1];
    if (value === undefined || value.startsWith("-")) {
      throw new UserFacingError(`${arg} requires a value`, {
        fix: `Run \`npm create actantdb@latest my-agent -- ${arg} <value>\`.`,
      });
    }
    valueArg(out, value);
    return index + 1;
  }

  const assigned = splitAssignedArg(arg);
  if (assigned) {
    assigned.parser(out, assigned.value);
  } else if (!arg.startsWith("-")) {
    out.positional.push(arg);
  } else {
    throw new UserFacingError(`unknown option: ${arg}`, {
      fix: "Run `npm create actantdb@latest -- --help` to see supported options.",
    });
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
  validateProjectName(name);

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
  if (!template) throw unknownTemplate(tplChoice);

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

  const normalizedFramework = normalizeFramework(framework);
  const normalizedLanguage = normalizeLanguage(language ?? "ts");
  const studioPort = normalizePort(args.studioPort ?? DEFAULT_STUDIO_PORT);

  return {
    projectName: name,
    template: template.id,
    framework: normalizedFramework,
    language: normalizedLanguage,
    studioPort,
  };
}

function nonInteractiveFlow(args: Args): ScaffoldChoices {
  const name = args.positional[0];
  validateProjectName(name);
  const template = args.template ?? "minimal";
  const templateDef = getTemplate(template);
  if (!templateDef) throw unknownTemplate(template);
  const framework = normalizeFramework(args.framework ?? templateDef.defaultFramework);
  const language = normalizeLanguage(args.language ?? "ts");
  const studioPort = normalizePort(args.studioPort ?? DEFAULT_STUDIO_PORT);
  return {
    projectName: name,
    template,
    framework,
    language,
    studioPort,
  };
}

function validateProjectName(name: string | undefined): asserts name is string {
  if (!name) {
    throw new UserFacingError("project name is required", {
      fix: "Run `npm create actantdb@latest my-agent -- --yes`.",
    });
  }
  if (!validProjectName(name)) {
    throw new UserFacingError(`invalid project name: "${name}"`, {
      detail: "Project names must start with a lowercase letter or digit and use only lowercase letters, digits, and dashes.",
      fix: "Try a name like `support-agent` or `my-agent`.",
    });
  }
}

function validProjectName(name: string): boolean {
  return /^[a-z0-9][a-z0-9-]*$/.test(name);
}

function unknownTemplate(template: string | undefined): UserFacingError {
  return new UserFacingError(`unknown template: ${template ?? ""}`, {
    detail: `Available templates: ${TEMPLATES.map((t) => t.id).join(", ")}.`,
    fix: "Run `npm create actantdb@latest my-agent -- --template minimal --yes`.",
  });
}

function normalizeFramework(framework: string | undefined): FrameworkChoice {
  if (FRAMEWORKS.some((f) => f.id === framework)) return framework as FrameworkChoice;
  throw new UserFacingError(`unknown framework: ${framework ?? ""}`, {
    detail: `Available frameworks: ${FRAMEWORKS.map((f) => f.id).join(", ")}.`,
    fix: "Use `--framework hand-rolled` for the smallest first run.",
  });
}

function normalizeLanguage(language: string | undefined): LanguageChoice {
  if (language === "ts" || language === "js") return language;
  throw new UserFacingError(`unknown language: ${language ?? ""}`, {
    detail: "Available languages: ts, js.",
    fix: "Use `--language js` for the fewest moving parts.",
  });
}

function normalizePort(port: number): number {
  if (Number.isInteger(port) && port > 0 && port < 65536) return port;
  throw new UserFacingError(`invalid port: ${String(port)}`, {
    detail: "Studio ports must be whole numbers between 1 and 65535.",
    fix: "Use `--port 4173` or omit the flag.",
  });
}

function formatError(err: unknown): string {
  const e = err instanceof Error ? err : new Error(String(err));
  const lines = [kleur.red(`error: ${e.message}`)];
  if (e instanceof UserFacingError) {
    if (e.detail) lines.push(kleur.gray(e.detail));
    if (e.fix) lines.push(kleur.cyan(`fix: ${e.fix}`));
  } else {
    lines.push(kleur.cyan("fix: Run `npm create actantdb@latest -- --help`, then retry with `--yes` for the golden path."));
  }
  return `${lines.join("\n")}\n`;
}

export async function main(argv: string[] = process.argv.slice(2)): Promise<number> {
  let args: Args;
  try {
    args = parseArgs(argv);
  } catch (err) {
    process.stderr.write(formatError(err));
    return 1;
  }
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
    process.stderr.write(formatError(err));
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
        `  npm run doctor`,
        "",
        kleur.gray("Files written:"),
        ...result.filesWritten.map((f) => `  ${f}`),
        "",
      ].join("\n"),
    );
    return 0;
  } catch (err) {
    process.stderr.write(formatError(err));
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
      process.stderr.write(formatError(err));
      process.exit(1);
    },
  );
}
