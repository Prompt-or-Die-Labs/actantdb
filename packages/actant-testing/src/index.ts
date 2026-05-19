/**
 * @actantdb/testing — assertions and helpers for consumer test suites.
 *
 * The shape mirrors how Jest/Vitest matchers compose: small functions that
 * throw on failure so they slot into any test runner. Built directly on
 * `@actantdb/core` — no peer test framework required.
 *
 *   import { createTestLedger, expectEventEmitted, expectGuardVerdict }
 *     from "@actantdb/testing";
 *
 *   const t = createTestLedger();
 *   // ... drive your agent against t.ledger ...
 *   expectEventEmitted(t, "tool_call_completed", { tool_name: "issue_refund" });
 *   expectGuardVerdict(t, { tool_name: "issue_refund", decision: "require_approval" });
 *
 * Snapshot-style: `t.snapshot()` returns a stable, payload-pruned list you
 * can pass to `toMatchInlineSnapshot()` etc.
 */

import { Ledger, ulid } from "@actantdb/core";
import type {
  ActantEvent,
  ApprovalDecision,
  ApprovalRequest,
  ContextManifest,
  EventKind,
  ModelCall,
  PolicyVerdict,
  ToolCallCompleted,
  ToolCallRequest,
} from "@actantdb/types";

/** Stable, redacted view of a ledger event used for snapshot diffs. */
export interface SnapshotEntry {
  kind: EventKind;
  run_id: string;
  parent_event_id?: string;
  sensitivity: string;
  payload: unknown;
}

/** Handle returned by `createTestLedger`. */
export interface TestLedger {
  /** The underlying in-memory ledger. Pass this where your consumer expects one. */
  readonly ledger: Ledger;
  /** Quick handle to append a new run id. */
  newRun(): string;
  /** Helpers wrapping `ledger.append(...)` so tests can seed events fluently. */
  appendUserMessage(runId: string, text: string): ActantEvent;
  appendModelCall(runId: string, info: ModelCall): ActantEvent;
  appendContextBuild(runId: string, manifest: ContextManifest): ActantEvent;
  appendToolCallRequested(runId: string, req: ToolCallRequest): ActantEvent;
  appendGuardVerdict(runId: string, toolCallId: string, verdict: PolicyVerdict): ActantEvent;
  appendApprovalRequired(runId: string, req: ApprovalRequest): ActantEvent;
  appendApprovalDecision(
    runId: string,
    toolCallId: string,
    decision: ApprovalDecision,
  ): ActantEvent;
  appendToolCallCompleted(runId: string, payload: ToolCallCompleted): ActantEvent;
  /** Snapshot, with optional payload field pruning. */
  snapshot(opts?: SnapshotOptions): SnapshotEntry[];
  /** Convenience: all events in the ledger, in causal order. */
  events(): ActantEvent[];
  /** Tear down (closes the underlying SQLite handle). */
  close(): void;
}

export interface CreateTestLedgerOptions {
  /** Project name. Default: `test-<random>`. */
  project?: string;
}

export interface SnapshotOptions {
  /** Drop these keys from every payload before comparison. */
  scrubKeys?: string[];
  /** Replace `created_at` / `chain_hash` style fields with sentinels. */
  stableTime?: boolean;
}

/** Default scrub list — id-ish + time-ish fields that change every run. */
const DEFAULT_SCRUB_KEYS = ["timestamp", "duration_ms", "trace_id", "span_id"];

/** Construct an in-memory test ledger. */
export function createTestLedger(opts: CreateTestLedgerOptions = {}): TestLedger {
  const project = opts.project ?? `test-${ulid().slice(0, 8).toLowerCase()}`;
  const ledger = new Ledger({ project, inMemory: true });

  function newRun(): string {
    const runId = ulid();
    ledger.append({
      kind: "agent_run_started",
      runId,
      payload: { project, meta: null },
      sensitivity: "low",
    });
    return runId;
  }

  return {
    ledger,
    newRun,
    appendUserMessage: (runId, text) =>
      ledger.append({
        kind: "user_message_received",
        runId,
        payload: { text },
        sensitivity: "low",
      }),
    appendModelCall: (runId, info) =>
      ledger.append({ kind: "model_call", runId, payload: info, sensitivity: "low" }),
    appendContextBuild: (runId, manifest) =>
      ledger.append({ kind: "context_build", runId, payload: manifest, sensitivity: "medium" }),
    appendToolCallRequested: (runId, req) =>
      ledger.append({ kind: "tool_call_requested", runId, payload: req, sensitivity: "low" }),
    appendGuardVerdict: (runId, toolCallId, verdict) =>
      ledger.append({
        kind: "guard_verdict",
        runId,
        payload: { tool_call_id: toolCallId, ...verdict },
        sensitivity: "low",
      }),
    appendApprovalRequired: (runId, req) =>
      ledger.append({
        kind: "approval_required",
        runId,
        payload: req,
        sensitivity: "low",
      }),
    appendApprovalDecision: (runId, toolCallId, decision) =>
      ledger.append({
        kind: "approval_decision",
        runId,
        payload: { tool_call_id: toolCallId, ...decision },
        sensitivity: "low",
      }),
    appendToolCallCompleted: (runId, payload) =>
      ledger.append({ kind: "tool_call_completed", runId, payload, sensitivity: "low" }),
    snapshot: (snapOpts) => snapshotEvents(ledger.query({}), snapOpts),
    events: () => ledger.query({}),
    close: () => ledger.close(),
  };
}

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

/**
 * Match shape: partial deep-equal. The matcher object's keys must all exist
 * in the candidate with deep-equal values; extra keys on the candidate are
 * ignored. Use `null` to assert the key is literally null.
 */
export type EventMatcher = Record<string, unknown>;

/** Find every event of `kind` matching the optional payload matcher. */
export function findEvents(
  source: TestLedger | Ledger | ActantEvent[],
  kind: EventKind,
  payloadMatch?: EventMatcher,
): ActantEvent[] {
  const events = sourceEvents(source);
  return events.filter(
    (e) => e.kind === kind && (!payloadMatch || matchesPartial(e.payload, payloadMatch)),
  );
}

/** Assert at least one event of `kind` was emitted with the given payload subset. */
export function expectEventEmitted(
  source: TestLedger | Ledger | ActantEvent[],
  kind: EventKind,
  payloadMatch?: EventMatcher,
): ActantEvent {
  const matches = findEvents(source, kind, payloadMatch);
  if (matches.length === 0) {
    const events = sourceEvents(source);
    const kinds = summarizeKinds(events);
    throw new AssertionError(
      `expected event "${kind}"${
        payloadMatch ? " matching " + JSON.stringify(payloadMatch) : ""
      } to be emitted, but none was.\n` +
        `ledger contains ${events.length} events: ${kinds}`,
    );
  }
  return matches[0]!;
}

/** Assert NO event of `kind` matching the payload subset was emitted. */
export function expectEventNotEmitted(
  source: TestLedger | Ledger | ActantEvent[],
  kind: EventKind,
  payloadMatch?: EventMatcher,
): void {
  const matches = findEvents(source, kind, payloadMatch);
  if (matches.length > 0) {
    throw new AssertionError(
      `expected NO event "${kind}"${
        payloadMatch ? " matching " + JSON.stringify(payloadMatch) : ""
      } to be emitted, but found ${matches.length}.`,
    );
  }
}

/** Assert a guard_verdict was emitted for `tool_name` with the given decision. */
export function expectGuardVerdict(
  source: TestLedger | Ledger | ActantEvent[],
  expected: {
    tool_name?: string;
    decision?: PolicyVerdict["decision"];
    reason_includes?: string;
  },
): ActantEvent {
  const events = sourceEvents(source);
  // Pair each guard_verdict to the most recent tool_call_requested with the
  // same tool_call_id, so a `tool_name` filter behaves as the consumer expects.
  const reqByToolCallId = new Map<string, string>();
  for (const e of events) {
    if (e.kind === "tool_call_requested") {
      const p = e.payload as { tool_call_id?: string; tool?: string };
      if (p.tool_call_id && p.tool) reqByToolCallId.set(p.tool_call_id, p.tool);
    }
  }
  for (const e of events) {
    if (e.kind !== "guard_verdict") continue;
    const p = e.payload as {
      tool_call_id?: string;
      decision?: string;
      reason?: string;
    };
    if (expected.decision && p.decision !== expected.decision) continue;
    if (expected.tool_name) {
      const tool = p.tool_call_id ? reqByToolCallId.get(p.tool_call_id) : undefined;
      if (tool !== expected.tool_name) continue;
    }
    if (expected.reason_includes && !(p.reason ?? "").includes(expected.reason_includes)) continue;
    return e;
  }
  const verdicts = events.filter((e) => e.kind === "guard_verdict").length;
  throw new AssertionError(
    `expected guard_verdict matching ${JSON.stringify(expected)}, but none found ` +
      `(ledger has ${verdicts} guard_verdict events).`,
  );
}

/** Assert that a tool call was completed (`tool_call_completed` with status ok). */
export function expectToolCompleted(
  source: TestLedger | Ledger | ActantEvent[],
  expected: { tool_name?: string; status?: ToolCallCompleted["status"] } = {},
): ActantEvent {
  const events = sourceEvents(source);
  const reqByToolCallId = new Map<string, string>();
  for (const e of events) {
    if (e.kind === "tool_call_requested") {
      const p = e.payload as { tool_call_id?: string; tool?: string };
      if (p.tool_call_id && p.tool) reqByToolCallId.set(p.tool_call_id, p.tool);
    }
  }
  for (const e of events) {
    if (e.kind !== "tool_call_completed") continue;
    const p = e.payload as { tool_call_id?: string; status?: string };
    const tool = p.tool_call_id ? reqByToolCallId.get(p.tool_call_id) : undefined;
    if (expected.tool_name && tool !== expected.tool_name) continue;
    if (expected.status && p.status !== expected.status) continue;
    return e;
  }
  throw new AssertionError(
    `expected tool_call_completed matching ${JSON.stringify(expected)}, but none found.`,
  );
}

/** Assert the hash chain across the ledger is intact (each event hashes properly). */
export function expectChainIntact(source: TestLedger | Ledger | ActantEvent[]): void {
  const events = sourceEvents(source);
  for (const e of events) {
    if (typeof e.chain_hash !== "string" || e.chain_hash.length !== 64) {
      throw new AssertionError(
        `event ${e.id} has invalid chain_hash "${e.chain_hash}" (expected 64-hex-char)`,
      );
    }
    if (typeof e.payload_hash !== "string" || e.payload_hash.length !== 64) {
      throw new AssertionError(
        `event ${e.id} has invalid payload_hash "${e.payload_hash}" (expected 64-hex-char)`,
      );
    }
  }
}

// ---------------------------------------------------------------------------
// Snapshot helpers
// ---------------------------------------------------------------------------

/** Reduce a list of events to stable, payload-pruned snapshot entries. */
export function snapshotEvents(events: ActantEvent[], opts: SnapshotOptions = {}): SnapshotEntry[] {
  const scrub = new Set([...(opts.scrubKeys ?? []), ...DEFAULT_SCRUB_KEYS]);
  return events.map((e) => {
    const entry: SnapshotEntry = {
      kind: e.kind,
      run_id: e.run_id,
      sensitivity: e.sensitivity,
      payload: stripKeys(e.payload, scrub),
    };
    if (e.parent_event_id !== undefined && e.parent_event_id !== null) {
      entry.parent_event_id = e.parent_event_id;
    }
    return entry;
  });
}

function stripKeys(value: unknown, scrub: Set<string>): unknown {
  if (Array.isArray(value)) return value.map((v) => stripKeys(v, scrub));
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value as object)) {
      if (scrub.has(k)) continue;
      out[k] = stripKeys(v, scrub);
    }
    return out;
  }
  return value;
}

// ---------------------------------------------------------------------------
// internals
// ---------------------------------------------------------------------------

function sourceEvents(source: TestLedger | Ledger | ActantEvent[]): ActantEvent[] {
  if (Array.isArray(source)) return source;
  if (source instanceof Ledger) return source.query({});
  return source.events();
}

function matchesPartial(value: unknown, matcher: EventMatcher): boolean {
  if (!value || typeof value !== "object") return false;
  for (const [k, expected] of Object.entries(matcher)) {
    const actual = (value as Record<string, unknown>)[k];
    if (!deepEq(actual, expected)) return false;
  }
  return true;
}

function deepEq(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null) return false;
  if (typeof a !== typeof b) return false;
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) {
      if (!deepEq(a[i], b[i])) return false;
    }
    return true;
  }
  if (typeof a === "object" && typeof b === "object") {
    const ak = Object.keys(a as object);
    const bk = Object.keys(b as object);
    if (ak.length !== bk.length) return false;
    for (const k of ak) {
      if (!deepEq((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k])) {
        return false;
      }
    }
    return true;
  }
  return false;
}

function summarizeKinds(events: ActantEvent[]): string {
  const counts: Record<string, number> = {};
  for (const e of events) counts[e.kind] = (counts[e.kind] ?? 0) + 1;
  return (
    Object.entries(counts)
      .map(([k, n]) => `${k}=${n}`)
      .join(", ") || "(empty)"
  );
}

/** Thrown by every `expect*` helper on failure. */
export class AssertionError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "ActantdbAssertionError";
  }
}
