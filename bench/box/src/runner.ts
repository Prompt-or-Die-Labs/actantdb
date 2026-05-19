/**
 * Three load patterns mirroring upstash/benchmarks:
 *
 *   - sequential — one at a time, await each
 *   - staggered  — 200ms between launches
 *   - burst      — all N started simultaneously via Promise.all
 *
 * Each measurement is:
 *   t0 = now
 *   box = await Box.create({ storeRoot: <isolated tmp dir>, ... })
 *   await box.exec.command("echo ready")     // first interactive command
 *   tti_ms = now - t0
 *   await box.delete()                       // clean up so each run is fresh
 *
 * Cleanup invariant: every box gets its own subdirectory under one
 * scenario-scoped tmp root. The runner rm -rf's that root in a `finally`
 * even if measureTTI throws mid-flight — N=100 cold starts must not pollute
 * `~/.actantdb/boxes`.
 */

import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";

import { Box } from "@actantdb/box";

import { compositeScore } from "./composite.js";
import { computeStats, formatStats, type Stats } from "./stats.js";

export type ScenarioName = "sequential" | "staggered" | "burst";

export interface ScenarioResult {
  scenario: ScenarioName;
  n: number;
  startedAt: string;
  ttiMs: number[];
  ok: number;
  fail: number;
  stats: Stats;
  composite: number;
  /** Per-failure error messages (truncated). Empty when fail===0. */
  errors: string[];
}

export interface RunOptions {
  /** Number of boxes per scenario. */
  n: number;
  /** Stagger spacing for the staggered scenario (ms). */
  staggerMs?: number;
  /** Print per-measurement debug lines. */
  verbose?: boolean;
}

const DEFAULT_STAGGER_MS = 200;
const ECHO_CMD = "echo ready";

interface Outcome {
  ok: boolean;
  ttiMs?: number;
  error?: string;
}

/**
 * Measure one box's cold-start TTI. Each call gets its own storeRoot so the
 * boxes never collide. Cleanup is best-effort: a failed `box.delete()` is
 * logged but does not cause the outcome to be marked as "fail" if the TTI
 * itself was measured successfully (we already have the number we care about).
 */
async function measureTTI(scenarioRoot: string, index: number): Promise<Outcome> {
  const boxStoreRoot = join(scenarioRoot, `m${index}`);
  let box: Box | undefined;
  const t0 = performance.now();
  try {
    box = await Box.create({
      name: `bench-${index}`,
      storeRoot: boxStoreRoot,
    });
    await box.exec.command(ECHO_CMD);
    const ttiMs = performance.now() - t0;
    return { ok: true, ttiMs };
  } catch (err) {
    return { ok: false, error: truncErr(err) };
  } finally {
    if (box) {
      try {
        await box.delete();
      } catch {
        // best-effort: scenarioRoot rm -rf in `runScenario`'s finally
        // covers anything we leak.
      }
    }
  }
}

function truncErr(err: unknown): string {
  const msg = err instanceof Error ? err.message : String(err);
  return msg.length > 240 ? `${msg.slice(0, 240)}…` : msg;
}

async function sequential(scenarioRoot: string, n: number): Promise<Outcome[]> {
  const outcomes: Outcome[] = [];
  for (let i = 0; i < n; i++) {
    outcomes.push(await measureTTI(scenarioRoot, i));
  }
  return outcomes;
}

async function staggered(
  scenarioRoot: string,
  n: number,
  spacingMs: number,
): Promise<Outcome[]> {
  // Same as Upstash: launch index i at time t0 + i*200ms, each running
  // independently. Collect via Promise.all so we await the slowest tail.
  const t0 = performance.now();
  const tasks: Promise<Outcome>[] = [];
  for (let i = 0; i < n; i++) {
    const targetT = t0 + i * spacingMs;
    const wait = Math.max(0, targetT - performance.now());
    await sleep(wait);
    tasks.push(measureTTI(scenarioRoot, i));
  }
  return Promise.all(tasks);
}

async function burst(scenarioRoot: string, n: number): Promise<Outcome[]> {
  const tasks: Promise<Outcome>[] = [];
  for (let i = 0; i < n; i++) tasks.push(measureTTI(scenarioRoot, i));
  return Promise.all(tasks);
}

function sleep(ms: number): Promise<void> {
  return new Promise((res) => setTimeout(res, ms));
}

export async function runScenario(
  scenario: ScenarioName,
  opts: RunOptions,
): Promise<ScenarioResult> {
  const startedAt = new Date().toISOString();
  const scenarioRoot = mkdtempSync(
    join(tmpdir(), `actantdb-box-bench-${scenario}-`),
  );
  let outcomes: Outcome[] = [];
  try {
    switch (scenario) {
      case "sequential":
        outcomes = await sequential(scenarioRoot, opts.n);
        break;
      case "staggered":
        outcomes = await staggered(
          scenarioRoot,
          opts.n,
          opts.staggerMs ?? DEFAULT_STAGGER_MS,
        );
        break;
      case "burst":
        outcomes = await burst(scenarioRoot, opts.n);
        break;
    }
  } finally {
    try {
      rmSync(scenarioRoot, { recursive: true, force: true });
    } catch {
      // ignore — /tmp gets reaped
    }
  }

  const ttiMs: number[] = [];
  const errors: string[] = [];
  let ok = 0;
  let fail = 0;
  for (const o of outcomes) {
    if (o.ok && typeof o.ttiMs === "number") {
      ttiMs.push(o.ttiMs);
      ok++;
    } else {
      fail++;
      if (o.error) errors.push(o.error);
    }
  }
  const stats = computeStats(ttiMs);
  const composite = compositeScore({ ok, fail, stats });
  return {
    scenario,
    n: opts.n,
    startedAt,
    ttiMs,
    ok,
    fail,
    stats,
    composite,
    errors,
  };
}

/** Pretty-print a ScenarioResult block. */
export function reportScenario(result: ScenarioResult): void {
  const heading = `ActantDB Box (local) — ${result.scenario}, N=${result.n}`;
  process.stdout.write(`${heading}\n`);
  process.stdout.write(`  TTI:   ${formatStats(result.stats)}\n`);
  process.stdout.write(`  ok=${result.ok}/${result.ok + result.fail}  fail=${result.fail}\n`);
  process.stdout.write(`  Composite score: ${result.composite.toFixed(1)} / 100\n`);
  if (result.errors.length > 0) {
    const sample = result.errors.slice(0, 3);
    process.stdout.write(`  Sample errors:\n`);
    for (const e of sample) process.stdout.write(`    - ${e}\n`);
    if (result.errors.length > sample.length) {
      process.stdout.write(`    (+${result.errors.length - sample.length} more)\n`);
    }
  }
  process.stdout.write("\n");
}
