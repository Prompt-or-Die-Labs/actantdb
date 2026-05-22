import type { EventKind, Sensitivity } from "@actantdb/types";

export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };
export type JsonRecord = { [key: string]: JsonValue };

export interface SupabaseErrorLike {
  message: string;
  code?: string;
  details?: string;
  hint?: string;
}

export interface SupabaseResult<Row extends JsonValue> {
  data: Row | null;
  error: SupabaseErrorLike | null;
}

export interface SupabaseSelectBuilder {
  eq(column: string, value: string | number | boolean): SupabaseSelectBuilder;
  is(column: string, value: null): SupabaseSelectBuilder;
  order(column: string, options: { ascending: boolean }): SupabaseSelectBuilder;
  limit(count: number): SupabaseSelectBuilder;
  maybeSingle(): PromiseLike<SupabaseResult<JsonRecord>>;
}

export interface SupabaseTableLike {
  select(columns: string): SupabaseSelectBuilder;
  insert(values: JsonRecord | JsonRecord[]): PromiseLike<SupabaseResult<JsonValue>>;
  upsert?(
    values: JsonRecord | JsonRecord[],
    options?: { onConflict?: string; ignoreDuplicates?: boolean },
  ): PromiseLike<SupabaseResult<JsonValue>>;
}

export interface SupabaseClientLike {
  from(table: string): SupabaseTableLike;
}

export interface SupabaseActantOptions {
  supabase: SupabaseClientLike;
  project: string;
  workspaceId: string;
  actorId: string;
  sessionId?: string;
  table?: string;
  ensureIdentity?: boolean;
  actorKind?: "human" | "agent" | "subagent" | "model" | "tool" | "worker" | "system";
  actorDisplayName?: string;
}

export interface SupabaseRunOptions {
  runId?: string;
  meta?: JsonValue;
  sessionId?: string;
}

export interface SupabaseRunContext {
  readonly runId: string;
  readonly project: string;
  readonly workspaceId: string;
  readonly actorId: string;
  recordUserMessage(text: string): Promise<SupabaseEventRow>;
  recordModelCall(payload: JsonRecord): Promise<SupabaseEventRow>;
  recordContextBuild(payload: JsonRecord): Promise<SupabaseEventRow>;
  recordEffect(payload: JsonValue): Promise<SupabaseEventRow>;
  finish(payload?: JsonValue): Promise<SupabaseEventRow>;
}

export interface SupabaseActantHandle {
  readonly project: string;
  readonly workspaceId: string;
  readonly actorId: string;
  ensureIdentity(): Promise<void>;
  startRun(opts?: SupabaseRunOptions): Promise<SupabaseRunContext>;
  appendEvent(input: SupabaseAppendInput): Promise<SupabaseEventRow>;
}

export interface SupabaseAppendInput {
  eventType: EventKind;
  payload: JsonValue;
  sessionId?: string;
  parentEventId?: string;
  sensitivity?: Sensitivity;
}

export interface SupabaseEventRow {
  id: string;
  workspace_id: string;
  actor_id: string;
  session_id: string | null;
  parent_event_id: string | null;
  event_type: EventKind;
  causality_kind: "observation" | "intent" | "effect" | "control" | "audit";
  sensitivity: Sensitivity;
  payload_inline: JsonValue;
  payload_hash: string;
  event_hash: string;
  created_at: string;
}

export type SupabaseEdgeHandler = (
  request: Request,
  context: { run: SupabaseRunContext; actant: SupabaseActantHandle },
) => Response | Promise<Response>;

export interface WrappedSupabaseEdgeHandler {
  (request: Request): Promise<Response>;
  readonly actant: SupabaseActantHandle;
}

const GENESIS_HASH = "0".repeat(64);

export function createSupabaseActant(opts: SupabaseActantOptions): SupabaseActantHandle {
  const table = opts.table ?? "agent_event";
  const ensureIdentity = opts.ensureIdentity ?? true;

  async function ensureIdentityRows(): Promise<void> {
    if (!ensureIdentity) return;
    const now = new Date().toISOString();
    const workspace: JsonRecord = {
      id: opts.workspaceId,
      name: opts.project,
      created_at: now,
    };
    const actor: JsonRecord = {
      id: opts.actorId,
      workspace_id: opts.workspaceId,
      kind: opts.actorKind ?? "agent",
      display_name: opts.actorDisplayName ?? opts.actorId,
      created_at: now,
    };
    await upsertRequired(opts.supabase, "workspace", workspace);
    await upsertRequired(opts.supabase, "actor", actor);
  }

  async function appendEvent(input: SupabaseAppendInput): Promise<SupabaseEventRow> {
    await ensureIdentityRows();
    const sessionId = input.sessionId ?? opts.sessionId;
    const previous = await loadLastEvent(opts.supabase, table, opts.workspaceId, sessionId);
    const payloadHash = await sha256Hex(canonicalJSON(input.payload));
    const eventHash = await sha256Hex(`${previous.eventHash}:${payloadHash}`);
    const row: SupabaseEventRow = {
      id: eventId(),
      workspace_id: opts.workspaceId,
      actor_id: opts.actorId,
      session_id: sessionId ?? null,
      parent_event_id: input.parentEventId ?? previous.id,
      event_type: input.eventType,
      causality_kind: causalityFor(input.eventType),
      sensitivity: input.sensitivity ?? "low",
      payload_inline: input.payload,
      payload_hash: payloadHash,
      event_hash: eventHash,
      created_at: new Date().toISOString(),
    };
    const insert = await opts.supabase.from(table).insert(rowToInsert(row));
    if (insert.error) throw new Error(`Supabase insert ${table}: ${insert.error.message}`);
    return row;
  }

  async function startRun(runOpts: SupabaseRunOptions = {}): Promise<SupabaseRunContext> {
    const runId = runOpts.runId ?? eventId();
    const sessionId = runOpts.sessionId ?? opts.sessionId;
    await appendEvent({
      eventType: "agent_run_started",
      ...sessionField(sessionId),
      payload: {
        project: opts.project,
        meta: runOpts.meta ?? null,
      },
    });
    return makeRunContext({
      project: opts.project,
      workspaceId: opts.workspaceId,
      actorId: opts.actorId,
      runId,
      sessionId,
      appendEvent,
    });
  }

  return {
    project: opts.project,
    workspaceId: opts.workspaceId,
    actorId: opts.actorId,
    ensureIdentity: ensureIdentityRows,
    startRun,
    appendEvent,
  };
}

export function withActantSupabaseEdge(
  handler: SupabaseEdgeHandler,
  opts: SupabaseActantOptions,
): WrappedSupabaseEdgeHandler {
  const actant = createSupabaseActant(opts);
  const wrapped = async (request: Request): Promise<Response> => {
    const run = await actant.startRun({
      meta: requestMeta(request),
    });
    try {
      const response = await handler(request, { run, actant });
      await run.recordEffect({
        adapter: "supabase-edge",
        response: {
          status: response.status,
          ok: response.ok,
        },
      });
      await run.finish({ ok: true, status: response.status });
      return response;
    } catch (err) {
      await run.finish({
        ok: false,
        error: err instanceof Error ? err.message : String(err),
      });
      throw err;
    }
  };
  Object.defineProperty(wrapped, "actant", { value: actant });
  return wrapped as WrappedSupabaseEdgeHandler;
}

function makeRunContext(args: {
  project: string;
  workspaceId: string;
  actorId: string;
  runId: string;
  sessionId: string | undefined;
  appendEvent: (input: SupabaseAppendInput) => Promise<SupabaseEventRow>;
}): SupabaseRunContext {
  return {
    runId: args.runId,
    project: args.project,
    workspaceId: args.workspaceId,
    actorId: args.actorId,
    recordUserMessage: (text) =>
      args.appendEvent({
        eventType: "user_message_received",
        ...sessionField(args.sessionId),
        payload: { run_id: args.runId, text },
      }),
    recordModelCall: (payload) =>
      args.appendEvent({
        eventType: "model_call",
        ...sessionField(args.sessionId),
        payload: { run_id: args.runId, ...payload },
      }),
    recordContextBuild: (payload) =>
      args.appendEvent({
        eventType: "context_build",
        ...sessionField(args.sessionId),
        payload: { run_id: args.runId, ...payload },
        sensitivity: "medium",
      }),
    recordEffect: (payload) =>
      args.appendEvent({
        eventType: "effect_observed",
        ...sessionField(args.sessionId),
        payload: { run_id: args.runId, value: payload },
      }),
    finish: (payload = {}) =>
      args.appendEvent({
        eventType: "agent_run_finished",
        ...sessionField(args.sessionId),
        payload: { run_id: args.runId, ...objectPayload(payload) },
      }),
  };
}

function sessionField(sessionId: string | undefined): { sessionId: string } | {} {
  return sessionId === undefined ? {} : { sessionId };
}

async function loadLastEvent(
  supabase: SupabaseClientLike,
  table: string,
  workspaceId: string,
  sessionId: string | undefined,
): Promise<{ id: string | null; eventHash: string }> {
  let query = supabase
    .from(table)
    .select("id,event_hash")
    .eq("workspace_id", workspaceId);
  query = sessionId ? query.eq("session_id", sessionId) : query.is("session_id", null);
  const result = await query
    .order("created_at", { ascending: false })
    .order("id", { ascending: false })
    .limit(1)
    .maybeSingle();
  if (result.error) throw new Error(`Supabase select ${table}: ${result.error.message}`);
  if (!result.data) return { id: null, eventHash: GENESIS_HASH };
  const eventHash = result.data.event_hash;
  const id = result.data.id;
  if (typeof eventHash !== "string" || typeof id !== "string") {
    throw new Error(`Supabase ${table} row missing string id/event_hash`);
  }
  return { id, eventHash };
}

async function upsertRequired(
  supabase: SupabaseClientLike,
  table: "workspace" | "actor",
  row: JsonRecord,
): Promise<void> {
  const target = supabase.from(table);
  if (!target.upsert) {
    throw new Error(`Supabase table ${table} must support upsert when ensureIdentity=true`);
  }
  const result = await target.upsert(row, { onConflict: "id", ignoreDuplicates: true });
  if (result.error) throw new Error(`Supabase upsert ${table}: ${result.error.message}`);
}

function requestMeta(request: Request): JsonRecord {
  const url = new URL(request.url);
  return {
    adapter: "supabase-edge",
    method: request.method,
    path: url.pathname,
  };
}

function objectPayload(value: JsonValue): JsonRecord {
  if (value && typeof value === "object" && !Array.isArray(value)) return value;
  return { value };
}

function rowToInsert(row: SupabaseEventRow): JsonRecord {
  return {
    id: row.id,
    workspace_id: row.workspace_id,
    actor_id: row.actor_id,
    session_id: row.session_id,
    parent_event_id: row.parent_event_id,
    event_type: row.event_type,
    causality_kind: row.causality_kind,
    sensitivity: row.sensitivity,
    payload_ref: null,
    payload_inline: row.payload_inline,
    payload_hash: row.payload_hash,
    event_hash: row.event_hash,
    created_at: row.created_at,
  };
}

function causalityFor(kind: EventKind): SupabaseEventRow["causality_kind"] {
  switch (kind) {
    case "agent_run_started":
    case "agent_run_finished":
    case "guard_verdict":
    case "approval_decision":
      return "control";
    case "tool_call_requested":
    case "approval_required":
      return "intent";
    case "tool_call_started":
    case "tool_call_completed":
    case "effect_observed":
      return "effect";
    case "context_build":
    case "model_call":
    case "user_message_received":
      return "observation";
  }
}

function eventId(): string {
  return `evt_${crypto.randomUUID().replaceAll("-", "")}`;
}

export function canonicalJSON(value: JsonValue): string {
  return JSON.stringify(sortJson(value));
}

function sortJson(value: JsonValue): JsonValue {
  if (Array.isArray(value)) return value.map(sortJson);
  if (value && typeof value === "object") {
    const out: JsonRecord = {};
    for (const key of Object.keys(value).sort()) {
      const child = value[key];
      if (child !== undefined) out[key] = sortJson(child);
    }
    return out;
  }
  return value;
}

export async function sha256Hex(text: string): Promise<string> {
  const encoded = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest("SHA-256", encoded);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

export const withActant = withActantSupabaseEdge;
