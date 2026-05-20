/**
 * @actantdb/box — schedule namespace.
 *
 * Zero new deps. Schedules are persisted to
 * `<workspaceDir>/.actantdb/schedules.json` so `Box.get(...)` can restore
 * them on process restart.
 *
 * Two kinds of schedule:
 *   - exec  → runs `box.exec.command(command)` every tick
 *   - agent → runs `box.agent.run({ prompt, timeout })` every tick
 *
 * Cron syntax: the most common 5-field forms are translated to a fixed
 * interval. Anything we can't parse falls back to a 60s tick. Callers who
 * need real cron should pass `everyMs` instead.
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { ulid, type Ledger } from "@actantdb/core";

import type { BoxAgentAPI } from "./agent.js";
import { BoxError } from "./errors.js";
import type { BoxExecAPI } from "./exec.js";
import type { Schedule } from "./types.js";

export interface ScheduleCtx {
  ledger: Ledger;
  workspaceDir: string;
}
type CtxProvider = () => ScheduleCtx;

const SCHEDULE_FILE = "schedules.json";

interface RuntimeSchedule extends Schedule {
  timer: NodeJS.Timeout | undefined;
}

export class BoxScheduleAPI {
  private readonly schedules = new Map<string, RuntimeSchedule>();

  constructor(
    private readonly ctx: CtxProvider,
    private readonly execApi: () => BoxExecAPI,
    private readonly agentApi: () => BoxAgentAPI,
  ) {}

  async exec(opts: { cron?: string; everyMs?: number; command: string }): Promise<Schedule> {
    return this.add({
      kind: "exec",
      command: opts.command,
      ...(opts.cron !== undefined ? { cron: opts.cron } : {}),
      ...(opts.everyMs !== undefined ? { everyMs: opts.everyMs } : {}),
    });
  }

  async agent(opts: {
    cron?: string;
    everyMs?: number;
    prompt: string;
    timeout?: number;
    options?: unknown;
  }): Promise<Schedule> {
    return this.add({
      kind: "agent",
      prompt: opts.prompt,
      ...(opts.cron !== undefined ? { cron: opts.cron } : {}),
      ...(opts.everyMs !== undefined ? { everyMs: opts.everyMs } : {}),
      ...(opts.timeout !== undefined ? { timeoutMs: opts.timeout } : {}),
    });
  }

  async list(): Promise<Schedule[]> {
    return Array.from(this.schedules.values()).map(stripTimer);
  }

  async get(id: string): Promise<Schedule> {
    const s = this.schedules.get(id);
    if (!s) throw new BoxError("schedule_not_found", `no schedule with id ${id}`);
    return stripTimer(s);
  }

  async pause(id: string): Promise<void> {
    const s = this.schedules.get(id);
    if (!s) throw new BoxError("schedule_not_found", `no schedule with id ${id}`);
    s.status = "paused";
    if (s.timer) clearInterval(s.timer);
    s.timer = undefined;
    this.persist();
  }

  async resume(id: string): Promise<void> {
    const s = this.schedules.get(id);
    if (!s) throw new BoxError("schedule_not_found", `no schedule with id ${id}`);
    s.status = "active";
    this.startTimer(s);
    this.persist();
  }

  async delete(id: string): Promise<void> {
    const s = this.schedules.get(id);
    if (!s) throw new BoxError("schedule_not_found", `no schedule with id ${id}`);
    if (s.timer) clearInterval(s.timer);
    this.schedules.delete(id);
    this.persist();
  }

  /** Pause every active schedule (called from `box.pause`). Does NOT mutate persisted state. */
  pauseAll(): void {
    for (const s of this.schedules.values()) {
      if (s.timer) clearInterval(s.timer);
      s.timer = undefined;
    }
  }

  /** Resume schedules whose persisted status is "active". */
  resumeAll(): void {
    for (const s of this.schedules.values()) {
      if (s.status === "active") this.startTimer(s);
    }
  }

  /** Stop everything (called from `box.delete`). */
  stopAll(): void {
    for (const s of this.schedules.values()) {
      if (s.timer) clearInterval(s.timer);
      s.timer = undefined;
    }
    this.schedules.clear();
  }

  /** Restore schedules from disk (called from Box.get). */
  async restore(): Promise<void> {
    const file = this.scheduleFile();
    if (!existsSync(file)) return;
    let raw: Schedule[];
    try {
      raw = JSON.parse(readFileSync(file, "utf8")) as Schedule[];
    } catch {
      return;
    }
    for (const s of raw) {
      const rt: RuntimeSchedule = { ...s, timer: undefined };
      this.schedules.set(rt.id, rt);
      if (rt.status === "active") this.startTimer(rt);
    }
  }

  // ----- internals -----

  private add(args: {
    kind: "exec" | "agent";
    cron?: string;
    everyMs?: number;
    command?: string;
    prompt?: string;
    timeoutMs?: number;
  }): Schedule {
    const id = `sched-${ulid()}`;
    const s: RuntimeSchedule = {
      id,
      kind: args.kind,
      ...(args.cron !== undefined ? { cron: args.cron } : {}),
      ...(args.everyMs !== undefined ? { everyMs: args.everyMs } : {}),
      ...(args.command !== undefined ? { command: args.command } : {}),
      ...(args.prompt !== undefined ? { prompt: args.prompt } : {}),
      ...(args.timeoutMs !== undefined ? { timeoutMs: args.timeoutMs } : {}),
      status: "active",
      createdAt: new Date().toISOString(),
      runs: 0,
      timer: undefined,
    };
    this.schedules.set(id, s);
    this.startTimer(s);
    this.persist();
    return stripTimer(s);
  }

  private startTimer(s: RuntimeSchedule): void {
    if (s.timer) clearInterval(s.timer);
    const interval = resolveIntervalMs(s);
    s.timer = setInterval(() => {
      void this.fire(s);
    }, interval);
    // Don't keep the event loop alive just for the schedule.
    s.timer.unref?.();
  }

  private async fire(s: RuntimeSchedule): Promise<void> {
    if (!this.schedules.has(s.id)) return;
    const { ledger } = this.ctx();
    s.runs += 1;
    s.lastFiredAt = new Date().toISOString();
    try {
      ledger.append({
        kind: "effect_observed",
        runId: `sched-${s.id}`,
        payload: {
          kind: "schedule_fired",
          schedule_id: s.id,
          schedule_kind: s.kind,
        },
        sensitivity: "low",
      });
    } catch {
      return;
    }
    try {
      if (s.kind === "exec" && s.command) {
        await this.execApi().command(s.command);
      } else if (s.kind === "agent" && s.prompt) {
        await this.agentApi().run({
          prompt: s.prompt,
          ...(s.timeoutMs !== undefined ? { timeout: s.timeoutMs } : {}),
        });
      }
    } catch (err) {
      if (!this.schedules.has(s.id)) return;
      try {
        ledger.append({
          kind: "effect_observed",
          runId: `sched-${s.id}`,
          payload: {
            kind: "schedule_failed",
            schedule_id: s.id,
            error: (err as Error).message ?? String(err),
          },
          sensitivity: "low",
        });
      } catch {
        return;
      }
    }
    if (!this.schedules.has(s.id)) return;
    this.persist();
  }

  private persist(): void {
    const file = this.scheduleFile();
    mkdirSync(join(this.ctx().workspaceDir, ".actantdb"), { recursive: true });
    const payload = Array.from(this.schedules.values()).map(stripTimer);
    writeFileSync(file, JSON.stringify(payload, null, 2), "utf8");
  }

  private scheduleFile(): string {
    return join(this.ctx().workspaceDir, ".actantdb", SCHEDULE_FILE);
  }
}

function stripTimer(s: RuntimeSchedule): Schedule {
  const { timer: _ignored, ...rest } = s;
  void _ignored;
  return rest;
}

/**
 * Translate a small subset of cron strings to an interval in ms.
 *
 * Supported forms (the rest fall back to 60_000ms):
 *   `* * * * *`      → 60s
 *   `*\/N * * * *`    → N * 60s
 *   `0 *\/N * * *`    → N * 3600s (hourly)
 *   `0 0 *\/N * *`    → N * 86400s (daily)
 *
 * If `everyMs` is provided we use it verbatim (preferred).
 */
function resolveIntervalMs(s: RuntimeSchedule): number {
  if (s.everyMs !== undefined && s.everyMs > 0) return s.everyMs;
  const cron = s.cron?.trim();
  if (!cron) return 60_000;
  const parts = cron.split(/\s+/);
  if (parts.length !== 5) return 60_000;
  const [minute, hour, dom, month, dow] = parts as [string, string, string, string, string];
  if (minute === "*" && hour === "*" && dom === "*" && month === "*" && dow === "*") return 60_000;
  const everyMatch = (s: string): number | null => {
    const m = s.match(/^\*\/(\d+)$/);
    return m ? Number(m[1]) : null;
  };
  if (hour === "*" && dom === "*" && month === "*" && dow === "*") {
    const n = everyMatch(minute);
    if (n) return n * 60_000;
  }
  if (minute === "0" && dom === "*" && month === "*" && dow === "*") {
    const n = everyMatch(hour);
    if (n) return n * 3_600_000;
  }
  if (minute === "0" && hour === "0" && month === "*" && dow === "*") {
    const n = everyMatch(dom);
    if (n) return n * 86_400_000;
  }
  return 60_000;
}
