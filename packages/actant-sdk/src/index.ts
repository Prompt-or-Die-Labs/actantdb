/**
 * @actantdb/sdk — TypeScript client for the ActantDB HTTP+WS server.
 *
 * Usage:
 *
 *   import { ActantClient } from "@actantdb/sdk";
 *   const client = new ActantClient({ baseUrl: "http://127.0.0.1:4555" });
 *   const r = await client.createSession({ workspaceId: "ws_x", actorId: "act_y" });
 */

import type {
  ActantEvent,
  ApprovalDecision,
  ApprovalRequest,
} from "@actantdb/types";

/** Options for the client. */
export interface ClientOptions {
  /** Base URL of the server (e.g. http://127.0.0.1:4555). */
  baseUrl: string;
  /** Bearer token (if the server is auth-protected). */
  token?: string;
  /** Default workspace id for convenience methods. */
  workspaceId?: string;
  /** Default actor id for convenience methods. */
  actorId?: string;
  /** Custom fetch implementation (for tests). */
  fetch?: typeof fetch;
}

/** Raw command request shape. */
export interface CommandRequest {
  workspaceId: string;
  actorId: string;
  commandType: string;
  input: unknown;
  idempotencyKey?: string;
}

/** Raw command response. */
export interface CommandResponse {
  command_id: string;
  event_id?: string | null;
  result: unknown;
}

/** Approval row as returned by /v1/approvals. */
export interface ApprovalRow {
  id: string;
  tool_call_id: string;
  requested_by: string;
  risk_level: string;
  summary: string;
  status: string;
}

/** ActantDB HTTP+WS client. */
export class ActantClient {
  private base: string;
  private fetcher: typeof fetch;
  private headers: Record<string, string>;

  constructor(opts: ClientOptions) {
    this.base = opts.baseUrl.replace(/\/+$/, "");
    this.fetcher = opts.fetch ?? fetch;
    this.headers = { "content-type": "application/json" };
    if (opts.token) this.headers["authorization"] = `Bearer ${opts.token}`;
  }

  /** Health check. */
  async healthz(): Promise<{ status: string; time: string }> {
    const r = await this.fetcher(`${this.base}/v1/healthz`);
    if (!r.ok) throw new Error(`healthz: ${r.status}`);
    return r.json() as Promise<{ status: string; time: string }>;
  }

  /** Dispatch any command. */
  async command(req: CommandRequest): Promise<CommandResponse> {
    const body = {
      workspace_id: req.workspaceId,
      actor_id: req.actorId,
      command_type: req.commandType,
      input: req.input,
      ...(req.idempotencyKey !== undefined
        ? { idempotency_key: req.idempotencyKey }
        : {}),
    };
    const r = await this.fetcher(`${this.base}/v1/command`, {
      method: "POST",
      headers: this.headers,
      body: JSON.stringify(body),
    });
    if (!r.ok) {
      const text = await r.text();
      throw new Error(`command ${req.commandType}: ${r.status} ${text}`);
    }
    return r.json() as Promise<CommandResponse>;
  }

  /** Convenience: alpha command shortcuts. */
  async createSession(opts: {
    workspaceId: string;
    actorId: string;
    title?: string;
  }): Promise<{ sessionId: string; response: CommandResponse }> {
    const r = await this.command({
      workspaceId: opts.workspaceId,
      actorId: opts.actorId,
      commandType: "create_session",
      input: opts.title !== undefined ? { title: opts.title } : {},
    });
    const result = r.result as { session_id: string };
    return { sessionId: result.session_id, response: r };
  }

  async appendUserMessage(opts: {
    workspaceId: string;
    actorId: string;
    sessionId: string;
    text: string;
  }): Promise<CommandResponse> {
    return this.command({
      workspaceId: opts.workspaceId,
      actorId: opts.actorId,
      commandType: "append_user_message",
      input: { session_id: opts.sessionId, text: opts.text },
    });
  }

  async requestToolCall(opts: {
    workspaceId: string;
    actorId: string;
    sessionId: string;
    toolName: string;
    arguments: unknown;
  }): Promise<{ toolCallId: string; status: string; verdict: unknown; response: CommandResponse }> {
    const r = await this.command({
      workspaceId: opts.workspaceId,
      actorId: opts.actorId,
      commandType: "request_tool_call",
      input: {
        session_id: opts.sessionId,
        tool_name: opts.toolName,
        arguments: opts.arguments,
      },
    });
    const result = r.result as { tool_call_id: string; status: string; verdict: unknown };
    return {
      toolCallId: result.tool_call_id,
      status: result.status,
      verdict: result.verdict,
      response: r,
    };
  }

  async approveToolCall(opts: {
    workspaceId: string;
    actorId: string;
    toolCallId: string;
    scope?: string;
  }): Promise<CommandResponse> {
    return this.command({
      workspaceId: opts.workspaceId,
      actorId: opts.actorId,
      commandType: "approve_tool_call",
      input: {
        tool_call_id: opts.toolCallId,
        scope: opts.scope ?? "once",
      },
    });
  }

  async recordToolResult(opts: {
    workspaceId: string;
    actorId: string;
    toolCallId: string;
    result: unknown;
  }): Promise<CommandResponse> {
    return this.command({
      workspaceId: opts.workspaceId,
      actorId: opts.actorId,
      commandType: "record_tool_result",
      input: {
        tool_call_id: opts.toolCallId,
        result: opts.result,
      },
    });
  }

  /** List Chronicle events for a session. */
  async events(opts: { sessionId: string }): Promise<{ events: ActantEvent[] }> {
    const u = new URL(`${this.base}/v1/events`);
    u.searchParams.set("session_id", opts.sessionId);
    const r = await this.fetcher(u.toString());
    if (!r.ok) throw new Error(`events: ${r.status}`);
    return r.json() as Promise<{ events: ActantEvent[] }>;
  }

  /** List pending approvals. */
  async approvals(opts: { workspaceId: string }): Promise<{ approvals: ApprovalRow[] }> {
    const u = new URL(`${this.base}/v1/approvals`);
    u.searchParams.set("workspace_id", opts.workspaceId);
    const r = await this.fetcher(u.toString());
    if (!r.ok) throw new Error(`approvals: ${r.status}`);
    return r.json() as Promise<{ approvals: ApprovalRow[] }>;
  }

  /** Subscribe to a topic via WebSocket. Returns the raw WebSocket. */
  subscribe(opts: {
    workspaceId: string;
    sessionId?: string;
    kind?: string;
  }): WebSocket {
    const url = new URL(`${this.base.replace(/^http/, "ws")}/v1/ws`);
    url.searchParams.set("workspace_id", opts.workspaceId);
    if (opts.sessionId) url.searchParams.set("session_id", opts.sessionId);
    url.searchParams.set("kind", opts.kind ?? "events");
    return new WebSocket(url.toString());
  }

  /**
   * Subscribe and yield parsed messages as an async iterable. Closes the
   * socket when the iterator is dropped or returns. Use:
   *
   *   for await (const msg of client.subscribeIter({workspaceId: "ws_1"})) {
   *     console.log(msg.topic, msg.payload);
   *   }
   */
  subscribeIter(opts: {
    workspaceId: string;
    sessionId?: string;
    kind?: string;
    /** Optional abort signal to close the socket early. */
    signal?: AbortSignal;
  }): AsyncIterable<SubscriptionMessage> {
    const ws = this.subscribe(opts);
    const queue: SubscriptionMessage[] = [];
    const waiters: Array<(v: IteratorResult<SubscriptionMessage>) => void> = [];
    let closed = false;
    let error: unknown = null;

    const wake = () => {
      while (waiters.length > 0 && (queue.length > 0 || closed || error)) {
        const next = waiters.shift()!;
        if (error) {
          next({ value: undefined as unknown as SubscriptionMessage, done: true });
          continue;
        }
        if (queue.length === 0) {
          next({ value: undefined as unknown as SubscriptionMessage, done: true });
        } else {
          next({ value: queue.shift()!, done: false });
        }
      }
    };

    ws.addEventListener("message", (e: MessageEvent) => {
      try {
        const parsed = JSON.parse(typeof e.data === "string" ? e.data : "") as SubscriptionMessage;
        queue.push(parsed);
      } catch {
        // skip unparseable frames silently
      }
      wake();
    });
    ws.addEventListener("close", () => {
      closed = true;
      wake();
    });
    ws.addEventListener("error", (e: Event) => {
      error = e;
      closed = true;
      wake();
    });
    if (opts.signal) {
      opts.signal.addEventListener("abort", () => {
        closed = true;
        try {
          ws.close();
        } catch {
          // already closed
        }
        wake();
      });
    }

    return {
      [Symbol.asyncIterator](): AsyncIterator<SubscriptionMessage> {
        return {
          next(): Promise<IteratorResult<SubscriptionMessage>> {
            if (queue.length > 0) {
              return Promise.resolve({ value: queue.shift()!, done: false });
            }
            if (closed) {
              return Promise.resolve({
                value: undefined as unknown as SubscriptionMessage,
                done: true,
              });
            }
            return new Promise((resolve) => waiters.push(resolve));
          },
          return(): Promise<IteratorResult<SubscriptionMessage>> {
            closed = true;
            try {
              ws.close();
            } catch {
              // already closed
            }
            return Promise.resolve({
              value: undefined as unknown as SubscriptionMessage,
              done: true,
            });
          },
        };
      },
    };
  }
}

/** Message received over the WebSocket subscription. */
export interface SubscriptionMessage {
  /** Topic the message was published to. */
  topic: {
    workspace_id: string;
    session_id?: string | null;
    kind: string;
  };
  /** Free-form payload (command response, event, etc.). */
  payload: unknown;
  /** RFC3339 publish time. */
  published_at: string;
}

export type { ActantEvent, ApprovalDecision, ApprovalRequest };
