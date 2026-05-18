import { createHash } from "node:crypto";

/** Canonical JSON: stable key order, no extra whitespace. */
export function canonicalJSON(value: unknown): string {
  return JSON.stringify(sortKeys(value));
}

function sortKeys(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortKeys);
  if (value && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>);
    entries.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
    return Object.fromEntries(entries.map(([k, v]) => [k, sortKeys(v)]));
  }
  return value;
}

/** SHA-256 of the canonical JSON of `value`, lowercase hex. */
export function sha256OfJSON(value: unknown): string {
  const h = createHash("sha256");
  h.update(canonicalJSON(value));
  return h.digest("hex");
}

/** SHA-256 of an arbitrary string, lowercase hex. */
export function sha256(text: string): string {
  return createHash("sha256").update(text).digest("hex");
}

/** Hash a (prev_chain_hash, payload_hash) pair into the next chain hash. */
export function nextChainHash(prevChainHash: string, payloadHash: string): string {
  return sha256(prevChainHash + ":" + payloadHash);
}
