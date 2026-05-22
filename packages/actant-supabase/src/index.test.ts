import { describe, expect, it } from "vitest";

import {
  createSupabaseActant,
  withActantSupabaseEdge,
  type JsonRecord,
  type JsonValue,
  type SupabaseClientLike,
  type SupabaseResult,
  type SupabaseSelectBuilder,
  type SupabaseTableLike,
} from "./index.js";

describe("@actantdb/supabase", () => {
  it("records a Supabase Edge invocation into agent_event", async () => {
    const supabase = new FakeSupabase();
    const handler = withActantSupabaseEdge(
      async (request, { run }) => {
        await run.recordUserMessage(`route ${new URL(request.url).pathname}`);
        await run.recordModelCall({
          model: "ollama:llama3.2:8b",
          role: "generator",
          prompt_hash: "hash",
          summary: "local response",
        });
        return new Response("ok", { status: 201 });
      },
      {
        supabase,
        project: "edge-test",
        workspaceId: "ws_edge",
        actorId: "act_edge",
      },
    );

    const response = await handler(new Request("https://example.supabase.co/functions/v1/agent"));

    expect(response.status).toBe(201);
    expect(supabase.rows("workspace")).toHaveLength(1);
    expect(supabase.rows("actor")).toHaveLength(1);
    const events = supabase.rows("agent_event");
    expect(events.map((row) => row.event_type)).toEqual([
      "agent_run_started",
      "user_message_received",
      "model_call",
      "effect_observed",
      "agent_run_finished",
    ]);
    expect(events[1]?.parent_event_id).toBe(events[0]?.id);
    expect(typeof events[0]?.event_hash).toBe("string");
  });

  it("records failed function completion before rethrowing", async () => {
    const supabase = new FakeSupabase();
    const handler = withActantSupabaseEdge(
      async () => {
        throw new Error("edge failed");
      },
      {
        supabase,
        project: "edge-test",
        workspaceId: "ws_edge",
        actorId: "act_edge",
      },
    );

    await expect(handler(new Request("https://example.supabase.co/functions/v1/fail"))).rejects.toThrow(
      "edge failed",
    );

    const last = supabase.rows("agent_event").at(-1);
    expect(last?.event_type).toBe("agent_run_finished");
    expect(last?.payload_inline).toMatchObject({ ok: false, error: "edge failed" });
  });

  it("can append explicit events when identity rows already exist", async () => {
    const supabase = new FakeSupabase();
    const actant = createSupabaseActant({
      supabase,
      project: "edge-test",
      workspaceId: "ws_edge",
      actorId: "act_edge",
      ensureIdentity: false,
    });

    const run = await actant.startRun({ meta: { source: "unit" } });
    await run.recordEffect({ ok: true });

    expect(supabase.rows("workspace")).toHaveLength(0);
    expect(supabase.rows("actor")).toHaveLength(0);
    expect(supabase.rows("agent_event").map((row) => row.event_type)).toEqual([
      "agent_run_started",
      "effect_observed",
    ]);
  });
});

type Filter =
  | { kind: "eq"; column: string; value: string | number | boolean }
  | { kind: "is"; column: string; value: null };

class FakeSupabase implements SupabaseClientLike {
  private readonly tables = new Map<string, JsonRecord[]>();

  from(table: string): SupabaseTableLike {
    if (!this.tables.has(table)) this.tables.set(table, []);
    return new FakeTable(this.rows(table));
  }

  rows(table: string): JsonRecord[] {
    const rows = this.tables.get(table);
    if (rows) return rows;
    const created: JsonRecord[] = [];
    this.tables.set(table, created);
    return created;
  }
}

class FakeTable implements SupabaseTableLike {
  constructor(private readonly rows: JsonRecord[]) {}

  select(_columns: string): SupabaseSelectBuilder {
    return new FakeSelect(this.rows);
  }

  async insert(values: JsonRecord | JsonRecord[]): Promise<SupabaseResult<JsonValue>> {
    const list = Array.isArray(values) ? values : [values];
    for (const value of list) this.rows.push({ ...value });
    return { data: null, error: null };
  }

  async upsert(
    values: JsonRecord | JsonRecord[],
    options: { onConflict?: string; ignoreDuplicates?: boolean } = {},
  ): Promise<SupabaseResult<JsonValue>> {
    const list = Array.isArray(values) ? values : [values];
    const key = options.onConflict ?? "id";
    for (const value of list) {
      const id = value[key];
      const existing = this.rows.findIndex((row) => row[key] === id);
      if (existing >= 0 && options.ignoreDuplicates) continue;
      if (existing >= 0) this.rows[existing] = { ...this.rows[existing], ...value };
      else this.rows.push({ ...value });
    }
    return { data: null, error: null };
  }
}

class FakeSelect implements SupabaseSelectBuilder {
  private readonly filters: Filter[] = [];
  private readonly orders: Array<{ column: string; ascending: boolean }> = [];
  private max = Number.POSITIVE_INFINITY;

  constructor(private readonly rows: JsonRecord[]) {}

  eq(column: string, value: string | number | boolean): SupabaseSelectBuilder {
    this.filters.push({ kind: "eq", column, value });
    return this;
  }

  is(column: string, value: null): SupabaseSelectBuilder {
    this.filters.push({ kind: "is", column, value });
    return this;
  }

  order(column: string, options: { ascending: boolean }): SupabaseSelectBuilder {
    this.orders.push({ column, ascending: options.ascending });
    return this;
  }

  limit(count: number): SupabaseSelectBuilder {
    this.max = count;
    return this;
  }

  async maybeSingle(): Promise<SupabaseResult<JsonRecord>> {
    const rows = this.rows
      .filter((row) => this.matches(row))
      .sort((left, right) => this.compare(left, right))
      .slice(0, this.max);
    return { data: rows[0] ?? null, error: null };
  }

  private matches(row: JsonRecord): boolean {
    return this.filters.every((filter) => {
      if (filter.kind === "eq") return row[filter.column] === filter.value;
      return row[filter.column] === null;
    });
  }

  private compare(left: JsonRecord, right: JsonRecord): number {
    for (const order of this.orders) {
      const leftValue = String(left[order.column] ?? "");
      const rightValue = String(right[order.column] ?? "");
      const result = leftValue.localeCompare(rightValue);
      if (result !== 0) return order.ascending ? result : -result;
    }
    return 0;
  }
}
