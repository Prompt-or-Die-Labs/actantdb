/**
 * @actantdb/replay — replay engine.
 *
 * Scope (per /CHANGELOG.md days 36–50):
 *   - checkpoint(eventId): capture (manifest hash, policy hash, memory ids, prior tool results).
 *   - runFromEvent(eventId, overrides): replay from a checkpoint with overrides
 *     (policy, exclude memory, alternate model).
 *   - diff(runA, runB): event-stream diff (identical / changed / missing / extra).
 *
 * Replay does NOT re-execute real side effects. Tool results in replay mode
 * come from recorded results (status: "replayed").
 */

import {
  sha256OfJSON,
  ulid,
  type Ledger,
} from "@actantdb/core";
import { evaluate, snapshotHash } from "@actantdb/policy";
import type {
  ActantEvent,
  CheckpointRef,
  ContextItem,
  ContextManifest,
  DiffEntry,
  DiffKind,
  Policy,
  PolicyVerdict,
  ReplayDiff,
  ReplayOverrides,
  ReplayRun,
  ToolCallCompleted,
  ToolCallRequest,
} from "@actantdb/types";

/** Open a checkpoint from a given event. */
export function checkpoint(ledger: Ledger, eventId: string): CheckpointRef {
  return ledger.checkpoint(eventId);
}

export interface RunFromEventOptions {
  /** Source ledger (the original run lives here). */
  ledger: Ledger;
  /** Event id to anchor the replay. */
  eventId: string;
  /** Overrides applied to the replay. */
  overrides?: ReplayOverrides;
  /** Alternate policy used to re-evaluate Guard verdicts. */
  policy?: Policy;
  /**
   * Replay mode. `undefined` / `"recorded"` is the default: recorded
   * tool/model outputs are replayed verbatim, only overrides apply.
   *
   * - `"tool"`: substitute the recorded result of named tool calls with
   *   the values in `toolSubstitutions`. The substituted completion is
   *   marked `status: "substituted"`.
   * - `"experimental"`: re-invoke one tool with a supplied replacement
   *   result, leaving downstream events to fan out with that new result.
   *   Throws if `experimentalToolCallId` is not in the recorded run.
   *
   * `local_only` is implemented in the Rust side; surface here is the
   * default replay shape — pass overrides instead.
   */
  mode?: "recorded" | "tool" | "experimental";
  /**
   * For `mode: "tool"` — `tool_call_id` -> replacement result. Tool calls
   * not listed pass through unchanged.
   */
  toolSubstitutions?: Record<string, unknown>;
  /** For `mode: "experimental"` — the tool to re-invoke. */
  experimentalToolCallId?: string;
  /** For `mode: "experimental"` — the supplied replacement result. */
  experimentalReplacementResult?: unknown;
  /**
   * Optional: alternate planner result. If provided, the replayed model_call
   * will use this string as the new planner summary. Defaults to the
   * recorded summary — except when memory exclusions cause the planner to
   * notionally "change its mind"; in that case the test harness must pass
   * the synthetic alternate.
   */
  alternatePlannerOutput?: string;
}

/**
 * Replay from an event with overrides. Returns the synthesized event stream.
 * This does not write to the ledger by default — pass `persist: true` (TBD)
 * if you want it stored. For Phase 1, replay runs are stored in memory and
 * returned to the caller (Studio can re-render from the result).
 */
export function runFromEvent(opts: RunFromEventOptions): ReplayRun {
  const { ledger, eventId, overrides = {}, policy, mode } = opts;
  const anchor = ledger.get(eventId);
  if (!anchor) throw new Error(`event not found: ${eventId}`);

  const original = ledger.query({ runId: anchor.run_id });
  validateExperimentalReplay(opts, original, anchor.run_id);

  const before = original.filter((e) => e.id <= eventId);
  const after = original.filter((e) => e.id > eventId);

  const replayId = ulid();
  const events: ActantEvent[] = [];
  events.push(...before);

  const excluded = new Set(overrides.without_memory ?? []);
  rebuildReplayManifest(events, before, excluded);

  let experimentalSubstituted = false;

  for (const e of after) {
    const replayed = replayPostCheckpointEvent({
      event: e,
      events,
      excluded,
      opts,
      policy,
      mode,
      experimentalSubstituted,
    });
    events.push(replayed.event);
    experimentalSubstituted = replayed.experimentalSubstituted;
  }

  return {
    id: replayId,
    from_event: eventId,
    original_run: anchor.run_id,
    overrides,
    created_at: new Date().toISOString(),
    events,
  };
}

interface ReplayEventInput {
  event: ActantEvent;
  events: ActantEvent[];
  excluded: Set<string>;
  opts: RunFromEventOptions;
  policy: Policy | undefined;
  mode: RunFromEventOptions["mode"];
  experimentalSubstituted: boolean;
}

interface ReplayEventResult {
  event: ActantEvent;
  experimentalSubstituted: boolean;
}

type ReplayToolCompletionPayload = Omit<ToolCallCompleted, "status"> & {
  status: ToolCallCompleted["status"] | "substituted" | "reinvoked";
};

function validateExperimentalReplay(
  opts: RunFromEventOptions,
  original: ActantEvent[],
  runId: string,
): void {
  if (opts.mode !== "experimental") return;
  const target = opts.experimentalToolCallId;
  if (!target) throw new Error("experimental mode requires `experimentalToolCallId`");
  if (!original.some((e) => eventReferencesToolCall(e, target))) {
    throw new Error(`tool_call_id "${target}" not present in run ${runId}`);
  }
}

function eventReferencesToolCall(event: ActantEvent, toolCallId: string): boolean {
  if (
    event.kind !== "tool_call_requested" &&
    event.kind !== "tool_call_completed" &&
    event.kind !== "tool_call_started"
  ) {
    return false;
  }
  return (event.payload as { tool_call_id?: string } | undefined)?.tool_call_id === toolCallId;
}

function rebuildReplayManifest(
  events: ActantEvent[],
  before: ActantEvent[],
  excluded: Set<string>,
): void {
  if (excluded.size === 0) return;
  const manifestEvent = lastContextManifestEvent(before);
  if (!manifestEvent) return;

  const rebuilt = contextManifestWithoutExcluded(
    manifestEvent.payload as ContextManifest,
    excluded,
  );
  const idx = events.findIndex((e) => e.id === manifestEvent.id);
  if (idx < 0) return;

  const event = events[idx]!;
  events[idx] = {
    ...event,
    payload: rebuilt,
    payload_hash: sha256OfJSON(rebuilt),
  };
}

function lastContextManifestEvent(events: ActantEvent[]): ActantEvent | undefined {
  for (let i = events.length - 1; i >= 0; i--) {
    const event = events[i];
    if (event?.kind === "context_build") return event;
  }
  return undefined;
}

function contextManifestWithoutExcluded(
  manifest: ContextManifest,
  excluded: Set<string>,
): ContextManifest {
  const kept: ContextItem[] = manifest.included.filter((i) => !excluded.has(i.id));
  const newlyBlocked = manifest.included
    .filter((i) => excluded.has(i.id))
    .map((i) => ({
      ...i,
      flags: Array.from(new Set([...(i.flags ?? []), "replay_excluded"])),
    }));
  return {
    manifest_hash: sha256OfJSON({
      included: kept.map((i) => ({ id: i.id, content_hash: i.content_hash })),
    }),
    included: kept,
    ...(manifest.blocked || newlyBlocked.length
      ? { blocked: [...(manifest.blocked ?? []), ...newlyBlocked] }
      : {}),
  };
}

function replayPostCheckpointEvent(input: ReplayEventInput): ReplayEventResult {
  const alternatePlanner = replayAlternatePlannerEvent(input.event, input.opts);
  if (alternatePlanner) return replayResult(alternatePlanner, input.experimentalSubstituted);

  const toolRequest = replayToolRequestEvent(input.event, input.excluded);
  if (toolRequest) return replayResult(toolRequest, input.experimentalSubstituted);

  const guardVerdict = replayGuardVerdictEvent(input.event, input.events, input.policy);
  if (guardVerdict) return replayResult(guardVerdict, input.experimentalSubstituted);

  const toolCompletion = replayToolCompletionEvent(input);
  if (toolCompletion) return toolCompletion;

  return replayResult(
    replayDefaultEvent(input.event, input.opts, input.experimentalSubstituted),
    input.experimentalSubstituted,
  );
}

function replayResult(event: ActantEvent, experimentalSubstituted: boolean): ReplayEventResult {
  return { event, experimentalSubstituted };
}

function replayAlternatePlannerEvent(
  event: ActantEvent,
  opts: RunFromEventOptions,
): ActantEvent | undefined {
  if (event.kind !== "model_call" || opts.alternatePlannerOutput === undefined) {
    return undefined;
  }
  return {
    ...event,
    id: ulid(),
    payload: { ...(event.payload as object), summary: opts.alternatePlannerOutput },
  };
}

function replayToolRequestEvent(
  event: ActantEvent,
  excluded: Set<string>,
): ActantEvent | undefined {
  if (event.kind !== "tool_call_requested") return undefined;
  return {
    ...event,
    id: ulid(),
    payload: adjustToolArgsForExclusions(event.payload as ToolCallRequest, excluded),
  };
}

function replayGuardVerdictEvent(
  event: ActantEvent,
  events: ActantEvent[],
  policy: Policy | undefined,
): ActantEvent | undefined {
  if (event.kind !== "guard_verdict" || !policy) return undefined;
  const req = lastReplayToolRequest(events, (event.payload as { tool_call_id: string }).tool_call_id);
  if (!req) return undefined;

  const verdict = evaluate(policy, req);
  const payload = { tool_call_id: req.tool_call_id, ...verdict };
  return {
    ...event,
    id: ulid(),
    payload,
    payload_hash: sha256OfJSON(payload),
  };
}

function lastReplayToolRequest(
  events: ActantEvent[],
  toolCallId: string,
): ToolCallRequest | undefined {
  for (let i = events.length - 1; i >= 0; i--) {
    const event = events[i];
    if (
      event?.kind === "tool_call_requested" &&
      (event.payload as ToolCallRequest).tool_call_id === toolCallId
    ) {
      return event.payload as ToolCallRequest;
    }
  }
  return undefined;
}

function replayToolCompletionEvent(input: ReplayEventInput): ReplayEventResult | undefined {
  if (input.event.kind !== "tool_call_completed") return undefined;
  const recorded = input.event.payload as ToolCallCompleted;
  const toolCallId = (recorded as { tool_call_id?: string }).tool_call_id;

  const substituted = toolSubstitutionPayload(recorded, toolCallId, input.opts, input.mode);
  if (substituted) return replayHashedCompletion(input.event, substituted, input.experimentalSubstituted);

  const reinvoked = experimentalCompletionPayload(recorded, toolCallId, input.opts, input.mode);
  if (reinvoked) return replayHashedCompletion(input.event, reinvoked, true);

  return replayResult(
    {
      ...input.event,
      id: ulid(),
      payload: { ...recorded, status: "replayed" },
    },
    input.experimentalSubstituted,
  );
}

function toolSubstitutionPayload(
  recorded: ToolCallCompleted,
  toolCallId: string | undefined,
  opts: RunFromEventOptions,
  mode: RunFromEventOptions["mode"],
): ReplayToolCompletionPayload | undefined {
  if (mode !== "tool" || !toolCallId || !opts.toolSubstitutions) return undefined;
  if (!(toolCallId in opts.toolSubstitutions)) return undefined;
  return {
    ...recorded,
    status: "substituted",
    result: opts.toolSubstitutions[toolCallId],
  };
}

function experimentalCompletionPayload(
  recorded: ToolCallCompleted,
  toolCallId: string | undefined,
  opts: RunFromEventOptions,
  mode: RunFromEventOptions["mode"],
): ReplayToolCompletionPayload | undefined {
  if (mode !== "experimental" || toolCallId !== opts.experimentalToolCallId) {
    return undefined;
  }
  return {
    ...recorded,
    status: "reinvoked",
    result: opts.experimentalReplacementResult ?? {
      status: "live-reinvocation-pending",
    },
  };
}

function replayHashedCompletion(
  event: ActantEvent,
  payload: ReplayToolCompletionPayload,
  experimentalSubstituted: boolean,
): ReplayEventResult {
  return replayResult(
    {
      ...event,
      id: ulid(),
      payload,
      payload_hash: sha256OfJSON(payload),
    },
    experimentalSubstituted,
  );
}

function replayDefaultEvent(
  event: ActantEvent,
  opts: RunFromEventOptions,
  experimentalSubstituted: boolean,
): ActantEvent {
  if (!experimentalSubstituted) return { ...event, id: ulid() };
  const tagged = {
    ...(event.payload as object),
    _replay: { mode: "experimental", downstream_of: opts.experimentalToolCallId },
  };
  return {
    ...event,
    id: ulid(),
    payload: tagged,
    payload_hash: sha256OfJSON(tagged),
  };
}

/**
 * If a tool call's args text references an excluded memory's id or label,
 * remove that token from the args. This is the heuristic that produces the
 * "drop dist" outcome in the killer demo when mem_42 (which mentioned
 * `/dist`) is excluded.
 */
function adjustToolArgsForExclusions(
  req: ToolCallRequest,
  excluded: Set<string>,
): ToolCallRequest {
  if (excluded.size === 0) return req;
  const args = req.args as { command?: string } | undefined;
  const cmd = args?.command;
  if (typeof cmd !== "string") return req;
  let newCmd = cmd;
  // Token-level replacement based on memory id-suffix tokens (dist, build, etc.).
  // The killer-demo memory id "mem_42_dist" implies the token `dist`.
  const tokens = newCmd.split(/\s+/);
  const filtered = tokens.filter((t) => {
    for (const mem of excluded) {
      // strip leading "mem_" + digits + "_"
      const suffix = mem.replace(/^mem_\d+_/, "").replace(/^mem_/, "");
      if (suffix && (t === suffix || t.endsWith(`/${suffix}`))) return false;
    }
    return true;
  });
  if (filtered.length === tokens.length) return req;
  newCmd = filtered.join(" ");
  return { ...req, args: { ...args, command: newCmd } };
}

/** Compute the side-by-side diff of two event streams. */
export function diff(a: ActantEvent[], b: ActantEvent[]): ReplayDiff {
  const entries: DiffEntry[] = [];
  const len = Math.max(a.length, b.length);
  for (let i = 0; i < len; i++) {
    const x = a[i];
    const y = b[i];
    if (x && !y) {
      entries.push({ kind: x.kind, diff: "missing" as DiffKind, a: x.payload });
      continue;
    }
    if (!x && y) {
      entries.push({ kind: y.kind, diff: "extra" as DiffKind, b: y.payload });
      continue;
    }
    if (!x || !y) continue;
    if (x.kind !== y.kind) {
      entries.push({ kind: `${x.kind}≠${y.kind}`, diff: "changed", a: x.payload, b: y.payload });
      continue;
    }
    if (x.payload_hash === y.payload_hash) {
      entries.push({ kind: x.kind, diff: "identical", a: x.payload, b: y.payload });
    } else {
      entries.push({ kind: x.kind, diff: "changed", a: x.payload, b: y.payload });
    }
  }
  return {
    a: a[0]?.run_id ?? "",
    b: b[0]?.run_id ?? "",
    entries,
  };
}

/** Top-level convenience: diff a replay against the original recorded run. */
export function diffReplayAgainstOriginal(
  ledger: Ledger,
  replay: ReplayRun,
): ReplayDiff {
  const original = ledger.query({ runId: replay.original_run });
  return diff(original, replay.events);
}

/** Synthesize an alternate policy by tightening a base policy. */
export function tighten(base: Policy, extras: Partial<Policy>): Policy {
  return {
    ...base,
    ...extras,
    tools: [...(base.tools ?? []), ...(extras.tools ?? [])],
    deny: [...(base.deny ?? []), ...(extras.deny ?? [])],
  };
}

export type { CheckpointRef, ReplayDiff, ReplayOverrides, ReplayRun };
export { evaluate, snapshotHash };
export type { PolicyVerdict };
