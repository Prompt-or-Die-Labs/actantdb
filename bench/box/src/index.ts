#!/usr/bin/env node
/**
 * `@actantdb-bench/box` — cold-start TTI benchmark CLI.
 *
 *   pnpm --filter @actantdb-bench/box bench
 *   pnpm --filter @actantdb-bench/box bench -- --scenario sequential --n 50
 *
 * Default: runs all three scenarios at N=100, writes one JSON per scenario
 * to bench/box/results/<scenario>-<unix>.json.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { hostname, platform, release } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { runScenario, reportScenario, type ScenarioName, type ScenarioResult } from "./runner.js";

const ALL_SCENARIOS: ScenarioName[] = ["sequential", "staggered", "burst"];
const DEFAULT_N = 100;
const DEFAULT_STAGGER_MS = 200;

function parseCli(argv: string[]): {
  scenarios: ScenarioName[];
  n: number;
  staggerMs: number;
  outDir: string;
} {
  const { values } = parseArgs({
    args: argv,
    options: {
      scenario: { type: "string", short: "s" },
      n: { type: "string" },
      stagger: { type: "string" },
      out: { type: "string", short: "o" },
      help: { type: "boolean", short: "h" },
    },
    allowPositionals: false,
    strict: true,
  });

  if (values.help) {
    process.stdout.write(usage());
    process.exit(0);
  }

  const scenarioArg = values.scenario;
  let scenarios: ScenarioName[];
  if (!scenarioArg || scenarioArg === "all") {
    scenarios = ALL_SCENARIOS;
  } else {
    if (!ALL_SCENARIOS.includes(scenarioArg as ScenarioName)) {
      throw new Error(
        `unknown --scenario '${scenarioArg}'. Valid: ${ALL_SCENARIOS.join(", ")}, all`,
      );
    }
    scenarios = [scenarioArg as ScenarioName];
  }

  const n = values.n ? Number.parseInt(values.n, 10) : DEFAULT_N;
  if (!Number.isFinite(n) || n <= 0) {
    throw new Error(`--n must be a positive integer, got '${values.n}'`);
  }
  const staggerMs = values.stagger
    ? Number.parseInt(values.stagger, 10)
    : DEFAULT_STAGGER_MS;
  if (!Number.isFinite(staggerMs) || staggerMs < 0) {
    throw new Error(`--stagger must be a non-negative integer, got '${values.stagger}'`);
  }

  // Default output dir: <this-file>/../../results, i.e. bench/box/results.
  const here = dirname(fileURLToPath(import.meta.url));
  const outDir = values.out ? resolve(process.cwd(), values.out) : resolve(here, "..", "results");

  return { scenarios, n, staggerMs, outDir };
}

function usage(): string {
  return `Usage: actantdb-box-bench [options]

Cold-start Time-To-Interactive (TTI) benchmark for @actantdb/box.
Shape-matched to upstash/benchmarks (sequential / staggered / burst).

Options:
  -s, --scenario <name>   sequential | staggered | burst | all   [default: all]
      --n <count>         boxes per scenario                      [default: 100]
      --stagger <ms>      staggered scenario spacing              [default: 200]
  -o, --out <dir>         results output dir                      [default: bench/box/results]
  -h, --help              show this message

Per-measurement TTI = t(Box.create) + t(first box.exec.command "echo ready").
Each box is created under an isolated tmp dir and deleted after measurement.
`;
}

function resolveBoxVersion(): string {
  // Pulled at runtime from the resolved @actantdb/box package.json. Wrapped
  // because the package may not expose its package.json via subpath exports;
  // on failure we report "unknown" rather than crashing the bench.
  try {
    const req = createRequire(import.meta.url);
    const pkg = req("@actantdb/box/package.json") as { version?: string };
    return pkg.version ?? "unknown";
  } catch {
    return "unknown";
  }
}

interface JsonRecord extends Omit<ScenarioResult, "ttiMs" | "stats" | "composite"> {
  tti_ms: number[];
  stats: ScenarioResult["stats"];
  composite: number;
  actantdb_box_version: string;
  node_version: string;
  platform: string;
  hostname: string;
}

function toJsonRecord(result: ScenarioResult): JsonRecord {
  return {
    scenario: result.scenario,
    n: result.n,
    startedAt: result.startedAt,
    ok: result.ok,
    fail: result.fail,
    errors: result.errors,
    tti_ms: result.ttiMs,
    stats: result.stats,
    composite: result.composite,
    actantdb_box_version: resolveBoxVersion(),
    node_version: process.version,
    platform: `${platform()} ${release()}`,
    hostname: hostname(),
  };
}

function writeResult(outDir: string, result: ScenarioResult): string {
  mkdirSync(outDir, { recursive: true });
  const unix = Math.floor(Date.parse(result.startedAt) / 1000);
  const path = resolve(outDir, `${result.scenario}-${unix}.json`);
  writeFileSync(path, JSON.stringify(toJsonRecord(result), null, 2), "utf8");
  return path;
}

async function main(): Promise<void> {
  // pnpm forwards a literal `--` separator on `pnpm run <script> -- --flag`.
  // node:util parseArgs treats `--` as positional and errors with
  // ERR_PARSE_ARGS_UNEXPECTED_POSITIONAL. Drop a leading `--` if present.
  let rawArgs = process.argv.slice(2);
  if (rawArgs[0] === "--") rawArgs = rawArgs.slice(1);
  const args = parseCli(rawArgs);
  process.stdout.write(
    `Running ${args.scenarios.length} scenario(s) at N=${args.n}; results -> ${args.outDir}\n\n`,
  );

  let hadFailures = false;
  for (const scenario of args.scenarios) {
    const opts = { n: args.n, staggerMs: args.staggerMs };
    const t0 = Date.now();
    const result = await runScenario(scenario, opts);
    const wallMs = Date.now() - t0;
    process.stdout.write(`(wall: ${wallMs}ms)\n`);
    reportScenario(result);
    const path = writeResult(args.outDir, result);
    process.stdout.write(`  → ${path}\n\n`);
    if (result.fail > 0) hadFailures = true;
  }

  if (hadFailures) {
    // Don't fail the process — the bench is observational by nature. Failures
    // surface in the JSON + report; CI can grep the workflow summary if it
    // wants to alert.
    process.stdout.write(
      "Note: one or more scenarios saw measurement failures; see JSON for details.\n",
    );
  }
}

main().catch((err) => {
  process.stderr.write(`bench failed: ${err instanceof Error ? err.stack ?? err.message : String(err)}\n`);
  process.exit(1);
});
