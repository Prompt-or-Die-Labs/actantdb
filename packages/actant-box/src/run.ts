/**
 * @actantdb/box — Run + StreamRun handles.
 *
 * Every Box operation that performs work returns a `Run`. The id maps 1:1 to
 * a logical run id in the underlying ledger so callers can drill into the
 * full event timeline via `box.ledger.query({ runId: run.id })` (Studio shows
 * the same view).
 *
 * Local mode never knows token counts; `cost.inputTokens` / `outputTokens`
 * are always 0 and `totalUsd` is always 0. `computeMs` is measured.
 */

import type { ActantEvent } from "@actantdb/types";
import type { Ledger } from "@actantdb/core";

import type { RunCost, RunStatus } from "./types.js";

export interface RunInit {
  id: string;
  ledger: Ledger;
  /** Optional cancellation hook (kill subprocess, clear timer, etc.). */
  cancel?: () => Promise<void> | void;
}

/**
 * Local Run handle. Mirrors Upstash Box's `Run` surface (`id`, `result`,
 * `status`, `cost`, `cancel`, `logs`) without depending on any cloud bits.
 */
export class Run {
  readonly id: string;
  result: unknown = undefined;
  status: RunStatus = "pending";
  cost: RunCost = { inputTokens: 0, outputTokens: 0, computeMs: 0, totalUsd: 0 };
  private readonly ledger: Ledger;
  private readonly cancelHook?: () => Promise<void> | void;
  private readonly startedAt: number;

  constructor(init: RunInit) {
    this.id = init.id;
    this.ledger = init.ledger;
    if (init.cancel) this.cancelHook = init.cancel;
    this.startedAt = performance.now();
  }

  /** Mark the run as running. */
  markRunning(): this {
    this.status = "running";
    return this;
  }

  /** Mark the run as complete with a result. Stamps cost.computeMs. */
  complete(result: unknown): this {
    this.result = result;
    this.status = "ok";
    this.cost = { ...this.cost, computeMs: Math.round(performance.now() - this.startedAt) };
    return this;
  }

  /** Mark the run as failed. */
  fail(err: unknown): this {
    this.result = err;
    this.status = "error";
    this.cost = { ...this.cost, computeMs: Math.round(performance.now() - this.startedAt) };
    return this;
  }

  /** Cancel the run via the optional cancel hook. */
  async cancel(): Promise<void> {
    if (this.status === "ok" || this.status === "error" || this.status === "cancelled") return;
    if (this.cancelHook) await this.cancelHook();
    this.status = "cancelled";
  }

  /** Return the ledger events for this run (chronological). */
  logs(): ActantEvent[] {
    return this.ledger.query({ runId: this.id });
  }
}
