/**
 * Distribution stats for a number[] of TTI measurements (ms).
 *
 * Uses nearest-rank percentile (the same conservative form used by the
 * upstash/benchmarks runner). Returns NaN on empty input rather than throwing
 * — the runner reports {ok, fail} separately so an empty observation list is
 * an existing failure-mode, not a fresh one.
 */
export interface Stats {
  min: number;
  max: number;
  median: number;
  p95: number;
  p99: number;
  mean: number;
}

export function computeStats(values: number[]): Stats {
  if (values.length === 0) {
    const nan = Number.NaN;
    return { min: nan, max: nan, median: nan, p95: nan, p99: nan, mean: nan };
  }
  const sorted = [...values].sort((a, b) => a - b);
  const n = sorted.length;
  const sum = sorted.reduce((acc, v) => acc + v, 0);
  return {
    min: sorted[0]!,
    max: sorted[n - 1]!,
    median: percentile(sorted, 0.5),
    p95: percentile(sorted, 0.95),
    p99: percentile(sorted, 0.99),
    mean: sum / n,
  };
}

/** Nearest-rank percentile on a *pre-sorted* ascending array. */
function percentile(sorted: number[], p: number): number {
  if (sorted.length === 0) return Number.NaN;
  // Ceiling rank, 1-indexed, clamp to last element.
  const rank = Math.max(1, Math.ceil(p * sorted.length));
  return sorted[Math.min(rank, sorted.length) - 1]!;
}

/** One-line formatter for `min=… median=… p95=… p99=… max=… mean=…` (ms). */
export function formatStats(s: Stats): string {
  const f = (v: number) => (Number.isFinite(v) ? `${v.toFixed(1)}ms` : "n/a");
  return [
    `min=${f(s.min)}`,
    `median=${f(s.median)}`,
    `p95=${f(s.p95)}`,
    `p99=${f(s.p99)}`,
    `max=${f(s.max)}`,
    `mean=${f(s.mean)}`,
  ].join("  ");
}
