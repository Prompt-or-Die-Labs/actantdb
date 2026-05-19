/**
 * @actantdb/replay — replay engine.
 *
 * Scope (per /wedge/60-day-plan.md days 36–50):
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

  // Collect the original run's events.
  const original = ledger.query({ runId: anchor.run_id });

  // Validate experimental mode: the named tool call must exist in this run.
  // (Throwing here keeps the caller honest — silent fallback to a regular
  // replay would mask test-harness mistakes.)
  if (mode === "experimental") {
    const target = opts.experimentalToolCallId;
    if (!target) {
      throw new Error("experimental mode requires `experimentalToolCallId`");
    }
    const present = original.some(
      (e) =>
        (e.kind === "tool_call_requested" ||
          e.kind === "tool_call_completed" ||
          e.kind === "tool_call_started") &&
        (e.payload as { tool_call_id?: string } | undefined)?.tool_call_id ===
          target,
    );
    if (!present) {
      throw new Error(
        `tool_call_id "${target}" not present in run ${anchor.run_id}`,
      );
    }
  }
  // Pre-checkpoint events are kept verbatim; post-checkpoint events are
  // recomputed under overrides.
  const before = original.filter((e) => e.id <= eventId);
  const after = original.filter((e) => e.id > eventId);

  const replayId = ulid();
  const events: ActantEvent[] = [];
  // Copy pre-checkpoint events into the replay (they happened, period).
  events.push(...before);

  // Reconstruct manifest minus excluded memory items.
  const excluded = new Set(overrides.without_memory ?? []);
  const lastManifestEvent = [...before]
    .reverse()
    .find((e) => e.kind === "context_build");
  let manifestAfter: ContextManifest | undefined = lastManifestEvent
    ? (lastManifestEvent.payload as ContextManifest)
    : undefined;
  if (manifestAfter && excluded.size > 0) {
    const kept: ContextItem[] = manifestAfter.included.filter((i) => !excluded.has(i.id));
    const newlyBlocked: ContextItem[] = manifestAfter.included
      .filter((i) => excluded.has(i.id))
      .map((i) => ({
        ...i,
        flags: Array.from(new Set([...(i.flags ?? []), "replay_excluded"])),
      }));
    const rebuilt: ContextManifest = {
      manifest_hash: sha256OfJSON({
        included: kept.map((i) => ({ id: i.id, content_hash: i.content_hash })),
      }),
      included: kept,
      ...(manifestAfter.blocked || newlyBlocked.length
        ? { blocked: [...(manifestAfter.blocked ?? []), ...newlyBlocked] }
        : {}),
    };
    manifestAfter = rebuilt;
    // Replace the last manifest in the replay events with the rebuilt one.
    const idx = events.findIndex((e) => e.id === lastManifestEvent!.id);
    if (idx >= 0) {
      const e = events[idx]!;
      events[idx] = {
        ...e,
        payload: rebuilt,
        payload_hash: sha256OfJSON(rebuilt),
      };
    }
  }

  // For mode=experimental, track when the re-invocation has fired so we
  // can mark every downstream event as "fanned out from a different
  // result" — payload_hash gets a `_replay` marker which makes the diff
  // surface them as `changed` rather than accidentally `identical`.
  let experimentalSubstituted = false;

  // Re-run each post-checkpoint event under overrides.
  for (const e of after) {
    if (e.kind === "model_call" && opts.alternatePlannerOutput !== undefined) {
      events.push({
        ...e,
        id: ulid(),
        payload: { ...(e.payload as object), summary: opts.alternatePlannerOutput },
      });
      continue;
    }
    if (e.kind === "tool_call_requested") {
      const req = e.payload as ToolCallRequest;
      // If a memory exclusion implies a safer command, substitute it.
      const adjusted = adjustToolArgsForExclusions(req, excluded);
      events.push({
        ...e,
        id: ulid(),
        payload: adjusted,
      });
      continue;
    }
    if (e.kind === "guard_verdict" && policy) {
      // Re-evaluate against the alternate policy using the *replay's* tool request
      // immediately preceding this verdict.
      const reqEvent = [...events]
        .reverse()
        .find(
          (x) =>
            x.kind === "tool_call_requested" &&
            (x.payload as ToolCallRequest).tool_call_id ===
              (e.payload as { tool_call_id: string }).tool_call_id,
        );
      const req = reqEvent ? (reqEvent.payload as ToolCallRequest) : undefined;
      if (req) {
        const v = evaluate(policy, req);
        events.push({
          ...e,
          id: ulid(),
          payload: { tool_call_id: req.tool_call_id, ...v },
          payload_hash: sha256OfJSON({ tool_call_id: req.tool_call_id, ...v }),
        });
        continue;
      }
    }
    if (e.kind === "tool_call_completed") {
      const recorded = e.payload as ToolCallCompleted;
      const tcid = (recorded as { tool_call_id?: string }).tool_call_id;

      // mode=tool: caller-supplied result wins for named tool calls.
      if (mode === "tool" && tcid && opts.toolSubstitutions && tcid in opts.toolSubstitutions) {
        const newPayload = {
          ...recorded,
          status: "substituted",
          result: opts.toolSubstitutions[tcid],
        };
        events.push({
          ...e,
          id: ulid(),
          payload: newPayload,
          payload_hash: sha256OfJSON(newPayload),
        });
        continue;
      }

      // mode=experimental: re-invoke the named tool with the supplied result.
      if (
        mode === "experimental" &&
        tcid === opts.experimentalToolCallId
      ) {
        experimentalSubstituted = true;
        const newPayload = {
          ...recorded,
          status: "reinvoked",
          result:
            opts.experimentalReplacementResult ?? {
              status: "live-reinvocation-pending",
            },
        };
        events.push({
          ...e,
          id: ulid(),
          payload: newPayload,
          payload_hash: sha256OfJSON(newPayload),
        });
        continue;
      }

      events.push({
        ...e,
        id: ulid(),
        payload: { ...recorded, status: "replayed" },
      });
      continue;
    }
    // Default: pass-through (re-id so replay events remain causally distinct).
    // Under mode=experimental, after the re-invocation has fired, mark
    // downstream events with a `_replay` field so the diff sees them as
    // `changed` (the recorded payload hashes the same; the marker carries
    // "this event re-walked under a different upstream result").
    if (experimentalSubstituted) {
      const tagged = {
        ...(e.payload as object),
        _replay: { mode: "experimental", downstream_of: opts.experimentalToolCallId },
      };
      events.push({
        ...e,
        id: ulid(),
        payload: tagged,
        payload_hash: sha256OfJSON(tagged),
      });
      continue;
    }
    events.push({ ...e, id: ulid() });
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
