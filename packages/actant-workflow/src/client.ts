/**
 * Client — programmatic counterpart to `serve()`.
 *
 *   const client = new Client({ baseUrl: "https://my-app/api/workflow" });
 *   await client.trigger({ body: { ... } });
 *   await client.cancel({ workflowRunId });
 *   await client.notify({ eventId, eventData });
 *
 * Two modes:
 *
 *   - HTTP mode (Upstash-equivalent): pass `baseUrl` and the client POSTs
 *     to your handler over fetch.
 *   - Local mode: pass `ledger` and the client writes directly to the
 *     ledger. This is what most ActantDB callers will use because the
 *     entire `@actantdb/*` stack composes through the shared ledger.
 *
 * The `token` option exists for Upstash compat and is ignored locally.
 */

import { ulid, type Ledger } from "@actantdb/core";

import { recordCancel, recordNotify } from "./runner.js";
import {
  WORKFLOW_RUN_ID_HEADER,
  type CancelArgs,
  type ClientOptions,
  type NotifyArgs,
  type TriggerArgs,
  type TriggerResult,
} from "./types.js";
import { WorkflowError } from "./errors.js";

export class Client {
  readonly baseUrl: string | undefined;
  readonly token: string | undefined;
  private readonly ledger: Ledger | undefined;
  private readonly fetchImpl: typeof globalThis.fetch;

  constructor(opts: ClientOptions = {}) {
    if (opts.baseUrl !== undefined) this.baseUrl = opts.baseUrl;
    if (opts.token !== undefined) this.token = opts.token;
    if (opts.ledger !== undefined) this.ledger = opts.ledger;
    this.fetchImpl = opts.fetch ?? globalThis.fetch.bind(globalThis);
  }

  /**
   * Start a new workflow run. Returns the run id so the caller can poll
   * status, cancel it, or notify into it later.
   */
  async trigger<P = unknown>(args: TriggerArgs<P>): Promise<TriggerResult> {
    const workflowRunId = args.workflowRunId ?? `wfr_${ulid()}`;
    const url = args.url ?? this.baseUrl;
    if (!url) {
      throw new WorkflowError(
        "invalid_request",
        "trigger requires `url` or Client({ baseUrl })",
      );
    }
    const baseHeaders: Record<string, string> = {
      "content-type": "application/json",
      [WORKFLOW_RUN_ID_HEADER]: workflowRunId,
      ...(this.token ? { authorization: `Bearer ${this.token}` } : {}),
    };
    const headers =
      args.headers === undefined ? baseHeaders : { ...baseHeaders, ...args.headers };
    const body = JSON.stringify({
      workflowRunId,
      body: args.body ?? null,
    });
    const res = await this.fetchImpl(url, {
      method: "POST",
      headers,
      body,
    });
    if (!res.ok && res.status !== 202) {
      throw new WorkflowError(
        "invalid_request",
        `trigger failed: ${res.status} ${res.statusText}`,
      );
    }
    return { workflowRunId };
  }

  /**
   * Cancel a run. In local mode, writes a `cancel` marker to the ledger;
   * the workflow's next step will throw `WorkflowError("run_cancelled")`
   * and `serve` will finalize the run with `status: "cancelled"`.
   */
  async cancel(args: CancelArgs): Promise<void> {
    if (this.ledger) {
      recordCancel(this.ledger, args.workflowRunId);
      return;
    }
    if (this.baseUrl) {
      const res = await this.fetchImpl(`${this.baseUrl}/cancel`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          ...(this.token ? { authorization: `Bearer ${this.token}` } : {}),
        },
        body: JSON.stringify({ workflowRunId: args.workflowRunId }),
      });
      if (!res.ok) {
        throw new WorkflowError(
          "invalid_request",
          `cancel failed: ${res.status} ${res.statusText}`,
        );
      }
      return;
    }
    throw new WorkflowError(
      "invalid_request",
      "cancel requires either Client({ ledger }) or Client({ baseUrl })",
    );
  }

  /**
   * Publish an event for any workflow currently `waitForEvent`-ing on
   * `eventId`. If `workflowRunId` is set, the event is scoped to that
   * run's ledger row; otherwise it is written under a synthetic run id
   * so any reader can find it (and the wait scan will pick it up by
   * `eventId` regardless of run).
   */
  async notify<D = unknown>(args: NotifyArgs<D>): Promise<void> {
    if (this.ledger) {
      const runId = args.workflowRunId ?? `wfr_notify_${ulid()}`;
      recordNotify(this.ledger, runId, args.eventId, args.eventData);
      return;
    }
    if (this.baseUrl) {
      const res = await this.fetchImpl(`${this.baseUrl}/notify`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          ...(this.token ? { authorization: `Bearer ${this.token}` } : {}),
        },
        body: JSON.stringify({
          eventId: args.eventId,
          eventData: args.eventData,
          ...(args.workflowRunId ? { workflowRunId: args.workflowRunId } : {}),
        }),
      });
      if (!res.ok) {
        throw new WorkflowError(
          "invalid_request",
          `notify failed: ${res.status} ${res.statusText}`,
        );
      }
      return;
    }
    throw new WorkflowError(
      "invalid_request",
      "notify requires either Client({ ledger }) or Client({ baseUrl })",
    );
  }
}
