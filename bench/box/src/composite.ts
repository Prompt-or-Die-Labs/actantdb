/**
 * Composite score, 0..100. Shape-matched to the upstash/benchmarks score
 * (success rate × latency penalty) but the *values* are this repo's formula
 * — Upstash's formula is not published, so we don't claim numerical parity.
 *
 * Formula (documented inline so reviewers can challenge it):
 *
 *   base       = 100 * success_rate            // 0..100
 *   penalty_ms = max(0, median_ms - FREE_MS)   // ms over the "free" budget
 *   penalty    = min(60, penalty_ms / PENALTY_DIVISOR_MS)
 *   score      = max(0, base - penalty)
 *
 * Defaults chosen so a 100% successful sequential run with median ≤ 100ms
 * scores ≥ 90, and a run with median 1000ms scores ~45 — i.e. the curve
 * separates "fast cloud container (1–2s TTI)" from "local subprocess
 * (10–50ms TTI)" without saturating at either end.
 *
 * The score is *only* meaningful when comparing runs of the same scenario
 * at the same N — different scenarios stress different parts of the system
 * (e.g. burst hits fs contention, sequential doesn't).
 */
import type { Stats } from "./stats.js";

const FREE_MS = 50; // anything under 50ms median = no penalty
const PENALTY_DIVISOR_MS = 20; // 20ms over the free budget = -1 point

export interface CompositeInput {
  ok: number;
  fail: number;
  stats: Stats;
}

export function compositeScore(input: CompositeInput): number {
  const total = input.ok + input.fail;
  if (total === 0) return 0;
  const successRate = input.ok / total;
  const base = 100 * successRate;
  const median = input.stats.median;
  if (!Number.isFinite(median)) return 0;
  const penaltyMs = Math.max(0, median - FREE_MS);
  const penalty = Math.min(60, penaltyMs / PENALTY_DIVISOR_MS);
  const score = Math.max(0, base - penalty);
  // Round to 1 decimal for stable presentation.
  return Math.round(score * 10) / 10;
}
