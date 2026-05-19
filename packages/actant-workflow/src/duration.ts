/**
 * Tiny duration parser. Accepts:
 *   - number (treated as ms)
 *   - "100ms", "5s", "5m", "3h", "7d"
 *
 * Anything else throws — keep this strict so typos don't silently turn into
 * 0-ms sleeps.
 */

import { WorkflowError } from "./errors.js";
import type { Duration } from "./types.js";

const UNITS: Record<string, number> = {
  ms: 1,
  s: 1000,
  m: 60_000,
  h: 3_600_000,
  d: 86_400_000,
};

export function parseDuration(d: Duration): number {
  if (typeof d === "number") {
    if (!Number.isFinite(d) || d < 0) {
      throw new WorkflowError("invalid_duration", `bad duration: ${d}`);
    }
    return Math.floor(d);
  }
  const m = /^(\d+)(ms|s|m|h|d)$/.exec(d.trim());
  if (!m) {
    throw new WorkflowError("invalid_duration", `bad duration: "${d}"`);
  }
  const n = Number.parseInt(m[1]!, 10);
  const unit = UNITS[m[2]!];
  if (unit === undefined) {
    throw new WorkflowError("invalid_duration", `bad duration unit: "${m[2]}"`);
  }
  return n * unit;
}

/** Accept ISO-8601 timestamp string OR unix-ms number. */
export function parseAbsoluteTime(t: string | number): number {
  if (typeof t === "number") {
    if (!Number.isFinite(t) || t < 0) {
      throw new WorkflowError("invalid_duration", `bad timestamp: ${t}`);
    }
    return Math.floor(t);
  }
  const ms = Date.parse(t);
  if (Number.isNaN(ms)) {
    throw new WorkflowError("invalid_duration", `bad ISO timestamp: "${t}"`);
  }
  return ms;
}
