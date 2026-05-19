/**
 * Runner — the "step skipping on resume" engine.
 *
 * On every invocation of a workflow handler, the workflow function is
 * called from the top. Each step asks the runner "has this step already
 * completed?":
 *
 *   - yes  → return the recorded result; the user's `fn` is never invoked.
 *   - no   → invoke `fn` live, write a `tool_call_completed` row, return.
 *
 * Sleeps and waitForEvents work the same way — they each write a marker
 * row on first encounter and on every subsequent invocation check whether
 * the precondition is satisfied. If not, they throw `WorkflowSuspended`
 * which `serve` catches.
 *
 * This file deliberately does NOT import the WorkflowContext (`context.ts`
 * depends on us, not the other way around) — the context is a thin facade
 * that calls these primitives.
 */

import type { Ledger } from "@actantdb/core";
import type { ActantEvent } from "@actantdb/types";

import { WorkflowError, WorkflowSuspended } from "./errors.js";
import type { CallArgs, CallResult } from "./types.js";

export type StepKind = "run" | "call" | "sleep" | "wait";

/** Marker rows we write to the ledger. */
interface RunMarker {
  step_kind: "run";
  name: string;
  result: unknown;
}
interface CallMarker {
  step_kind: "call";
  name: string;
  result: CallResult;
}
interface SleepMarker {
  kind: "sleep";
  name: string;
  /** Unix ms when the sleep is over. */
  until: number;
}
interface WaitMarker {
  kind: "wait";
  name: string;
  eventId: string;
  /** Unix ms after which we time out. Omitted = no timeout. */
  timeoutAt?: number;
}
interface EventMarker {
  kind: "event";
  eventId: string;
  data: unknown;
}
interface CancelMarker {
  kind: "cancel";
}

/**
 * Per-invocation state. A fresh one is built for every HTTP request
 * because the workflow function runs from the top each time.
 */
export interface RunnerState {
  ledger: Ledger;
  runId: string;
  /** Events at the moment the invocation started — never re-queried mid-run. */
  history: ActantEvent[];
  /** Set of step / sleep / wait names we've seen this invocation, for duplicate detection. */
  seenNames: Set<string>;
  /** `now()` clock — overridable for tests. */
  now: () => number;
  /** `fetch` impl — overridable for tests. */
  fetch: typeof globalThis.fetch;
}

export function makeState(
  ledger: Ledger,
  runId: string,
  opts: { now?: () => number; fetch?: typeof globalThis.fetch } = {},
): RunnerState {
  return {
    ledger,
    runId,
    history: ledger.query({ runId }),
    seenNames: new Set(),
    now: opts.now ?? (() => Date.now()),
    fetch: opts.fetch ?? globalThis.fetch.bind(globalThis),
  };
}

export function ensureNotCancelled(state: RunnerState): void {
  for (const e of state.history) {
    if (e.kind !== "effect_observed") continue;
    const p = e.payload as Partial<CancelMarker>;
    if (p && p.kind === "cancel") {
      throw new WorkflowError("run_cancelled", `run ${state.runId} was cancelled`);
    }
  }
}

function recordStepName(state: RunnerState, prefix: string, name: string): string {
  const key = `${prefix}:${name}`;
  if (state.seenNames.has(key)) {
    throw new WorkflowError(
      "duplicate_step",
      `duplicate step name "${name}" within run ${state.runId}`,
    );
  }
  state.seenNames.add(key);
  return key;
}

/** Find a `tool_call_completed` row whose tool_call_id matches `step:<name>` or `call:<name>`. */
function findStepResult(state: RunnerState, key: string): unknown | undefined {
  for (const e of state.history) {
    if (e.kind !== "tool_call_completed") continue;
    const p = e.payload as { tool_call_id?: string; result?: unknown } | undefined;
    if (p?.tool_call_id === key) return p.result;
  }
  return undefined;
}

/** Walk an `effect_observed` payload, returning the first matching marker by `kind` + `name`. */
function findEffect<T extends { kind: string; name?: string }>(
  state: RunnerState,
  kind: T["kind"],
  name?: string,
): T | undefined {
  for (const e of state.history) {
    if (e.kind !== "effect_observed") continue;
    const p = e.payload as Partial<T> | undefined;
    if (!p) continue;
    if (p.kind !== kind) continue;
    if (name !== undefined && p.name !== name) continue;
    return p as T;
  }
  return undefined;
}

/** Append a step result row. */
function appendStepResult(
  state: RunnerState,
  key: string,
  result: unknown,
): void {
  const ev = state.ledger.append({
    kind: "tool_call_completed",
    runId: state.runId,
    payload: {
      tool_call_id: key,
      status: "ok",
      result,
    },
  });
  state.history.push(ev);
}

function appendEffect(state: RunnerState, payload: unknown): void {
  const ev = state.ledger.append({
    kind: "effect_observed",
    runId: state.runId,
    payload,
  });
  state.history.push(ev);
}

// --------- primitives the context exposes ---------

export async function doRun<T>(
  state: RunnerState,
  name: string,
  fn: () => T | Promise<T>,
): Promise<T> {
  ensureNotCancelled(state);
  const key = recordStepName(state, "step", name);
  const cached = findStepResult(state, key);
  if (cached !== undefined) return cached as T;

  let result: T;
  try {
    result = await fn();
  } catch (err) {
    throw new WorkflowError(
      "step_threw",
      `step "${name}" threw: ${(err as Error)?.message ?? String(err)}`,
      err,
    );
  }
  appendStepResult(state, key, result);
  return result;
}

export async function doCall<R>(
  state: RunnerState,
  name: string,
  args: CallArgs,
): Promise<CallResult<R>> {
  ensureNotCancelled(state);
  const key = recordStepName(state, "call", name);
  const cached = findStepResult(state, key);
  if (cached !== undefined) return cached as CallResult<R>;

  const init: RequestInit = {
    method: args.method ?? "GET",
    ...(args.headers ? { headers: args.headers } : {}),
  };
  if (args.body !== undefined) {
    init.body =
      typeof args.body === "string" ? args.body : JSON.stringify(args.body);
    if (!args.headers || !("content-type" in lowercaseHeaders(args.headers))) {
      init.headers = {
        "content-type": "application/json",
        ...(args.headers ?? {}),
      };
    }
  }
  const res = await state.fetch(args.url, init);
  const headers: Record<string, string> = {};
  res.headers.forEach((v, k) => {
    headers[k] = v;
  });
  let body: unknown;
  const ct = res.headers.get("content-type") ?? "";
  if (ct.includes("application/json")) {
    body = await res.json();
  } else {
    body = await res.text();
  }
  const result: CallResult<R> = {
    status: res.status,
    headers,
    body: body as R,
  };
  appendStepResult(state, key, result);
  return result;
}

function lowercaseHeaders(h: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(h)) out[k.toLowerCase()] = v;
  return out;
}

export function doSleep(
  state: RunnerState,
  name: string,
  untilMs: number,
): void {
  ensureNotCancelled(state);
  const key = recordStepName(state, "sleep", name);
  void key;
  const existing = findEffect<SleepMarker>(state, "sleep", name);
  if (existing) {
    if (state.now() >= existing.until) return;
    throw new WorkflowSuspended("sleep", name, {
      retryAfterMs: Math.max(0, existing.until - state.now()),
    });
  }
  // First time we see this sleep — record it.
  appendEffect(state, { kind: "sleep", name, until: untilMs } satisfies SleepMarker);
  if (state.now() >= untilMs) return;
  throw new WorkflowSuspended("sleep", name, {
    retryAfterMs: Math.max(0, untilMs - state.now()),
  });
}

export function doWaitForEvent<D = unknown>(
  state: RunnerState,
  name: string,
  eventId: string,
  timeoutMs?: number,
): D | undefined {
  ensureNotCancelled(state);
  const key = recordStepName(state, "wait", name);
  void key;

  // Record the wait marker on first encounter.
  let marker = findEffect<WaitMarker>(state, "wait", name);
  if (!marker) {
    const m: WaitMarker = {
      kind: "wait",
      name,
      eventId,
      ...(timeoutMs !== undefined ? { timeoutAt: state.now() + timeoutMs } : {}),
    };
    appendEffect(state, m);
    marker = m;
  }

  // Is the matching event already in the ledger? Scan after the wait marker.
  const waitMarkerIdx = state.history.findIndex(
    (e) =>
      e.kind === "effect_observed" &&
      (e.payload as Partial<WaitMarker>)?.kind === "wait" &&
      (e.payload as Partial<WaitMarker>)?.name === name,
  );
  for (let i = Math.max(0, waitMarkerIdx); i < state.history.length; i++) {
    const e = state.history[i]!;
    if (e.kind !== "effect_observed") continue;
    const p = e.payload as Partial<EventMarker>;
    if (p?.kind === "event" && p.eventId === eventId) {
      return p.data as D;
    }
  }

  // Not yet — has the timeout expired?
  if (marker.timeoutAt !== undefined && state.now() >= marker.timeoutAt) {
    return undefined;
  }

  throw new WorkflowSuspended("wait_for_event", name, {
    waitingForEventId: eventId,
    ...(marker.timeoutAt !== undefined
      ? { retryAfterMs: Math.max(0, marker.timeoutAt - state.now()) }
      : {}),
  });
}

/**
 * Append a `cancel` effect_observed row. Safe to call from inside or outside
 * the workflow function.
 */
export function recordCancel(ledger: Ledger, runId: string): void {
  ledger.append({
    kind: "effect_observed",
    runId,
    payload: { kind: "cancel" } satisfies CancelMarker,
  });
}

/** Append a `notify` event row that `waitForEvent` will pick up. */
export function recordNotify(
  ledger: Ledger,
  runId: string,
  eventId: string,
  data: unknown,
): void {
  ledger.append({
    kind: "effect_observed",
    runId,
    payload: { kind: "event", eventId, data } satisfies EventMarker,
  });
}

export type {
  RunMarker,
  CallMarker,
  SleepMarker,
  WaitMarker,
  EventMarker,
  CancelMarker,
};
