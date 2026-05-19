/**
 * WorkflowContext — the `ctx` argument the consumer's handler receives.
 *
 * Mirrors Upstash's `WorkflowContext` shape so a port is a search-and-replace:
 *
 *   ctx.run(name, fn)                — durable step.
 *   ctx.sleep(name, duration)        — durable sleep.
 *   ctx.sleepUntil(name, time)       — sleep until absolute time.
 *   ctx.call(name, { url, ... })     — durable fetch.
 *   ctx.waitForEvent(name, eventId)  — pause until notify.
 *   ctx.notify(eventId, data)        — publish event from inside a workflow.
 *   ctx.cancel()                     — abort the current run.
 *
 * Plus a few read-only fields:
 *   ctx.runId, ctx.payload, ctx.requestHeaders.
 */

import type { Ledger } from "@actantdb/core";

import { WorkflowCancelled } from "./errors.js";
import { parseAbsoluteTime, parseDuration } from "./duration.js";
import {
  doCall,
  doRun,
  doSleep,
  doWaitForEvent,
  recordNotify,
  type RunnerState,
} from "./runner.js";
import type {
  CallArgs,
  CallResult,
  Duration,
  WaitForEventOptions,
} from "./types.js";

export class WorkflowContext<P = unknown> {
  readonly runId: string;
  readonly payload: P;
  readonly requestHeaders: Record<string, string>;
  private readonly state: RunnerState;
  private readonly ledger: Ledger;

  constructor(args: {
    runId: string;
    payload: P;
    requestHeaders?: Record<string, string>;
    state: RunnerState;
    ledger: Ledger;
  }) {
    this.runId = args.runId;
    this.payload = args.payload;
    this.requestHeaders = args.requestHeaders ?? {};
    this.state = args.state;
    this.ledger = args.ledger;
  }

  run<T>(name: string, fn: () => T | Promise<T>): Promise<T> {
    return doRun(this.state, name, fn);
  }

  sleep(name: string, duration: Duration): void {
    const ms = parseDuration(duration);
    doSleep(this.state, name, this.now() + ms);
  }

  sleepUntil(name: string, time: string | number): void {
    const at = parseAbsoluteTime(time);
    doSleep(this.state, name, at);
  }

  call<R = unknown, B = unknown>(
    name: string,
    args: CallArgs<B>,
  ): Promise<CallResult<R>> {
    return doCall<R>(this.state, name, args);
  }

  waitForEvent<D = unknown>(
    name: string,
    eventId: string,
    opts: WaitForEventOptions = {},
  ): D | undefined {
    const timeoutMs =
      opts.timeout !== undefined ? parseDuration(opts.timeout) : undefined;
    return doWaitForEvent<D>(this.state, name, eventId, timeoutMs);
  }

  /**
   * Convenience: publish an event into the ledger for any `waitForEvent`
   * listener (this run or another). Matches Upstash's `context.notify`.
   *
   * NOTE: in Upstash this can be awaited and returns event delivery
   * receipts; here it's fire-and-forget against the local ledger.
   */
  notify(eventId: string, data?: unknown): void {
    recordNotify(this.ledger, this.runId, eventId, data);
  }

  /**
   * Abort the current run. Writes a `cancel` marker and throws an internal
   * sentinel that the runner translates into a clean `status: "cancelled"`.
   */
  cancel(): never {
    this.ledger.append({
      kind: "effect_observed",
      runId: this.runId,
      payload: { kind: "cancel" },
    });
    throw new WorkflowCancelled();
  }

  private now(): number {
    return this.state.now();
  }
}

/** Handler type the consumer passes to `serve`. */
export type WorkflowHandler<P = unknown, R = unknown> = (
  ctx: WorkflowContext<P>,
) => R | Promise<R>;
