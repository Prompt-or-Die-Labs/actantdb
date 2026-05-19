import { mkdirSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";

import type {
  ActantEvent,
  EventKind,
  Sensitivity,
  CheckpointRef,
} from "@actantdb/types";

import { canonicalJSON, nextChainHash, sha256, sha256OfJSON } from "./hash.js";
import { ulid } from "./ulid.js";

const GENESIS_HASH = "0".repeat(64);

/** Options to open a Ledger. */
export interface LedgerOptions {
  /** Project identifier; used to scope the store path. */
  project: string;
  /** Absolute path to the SQLite file. If omitted, derived from project. */
  dbPath?: string;
  /** Root storage directory (overrides ~/.actantdb). */
  storeDir?: string;
  /**
   * Open the ledger entirely in RAM (uses node:sqlite's `:memory:`).
   *
   * Intended for tests, CI environments where `~/.actantdb` isn't writable,
   * and any case where you want a `Ledger` instance to be sharable across
   * an in-process agent and Studio without touching disk. Mutually
   * exclusive with `dbPath`/`storeDir` — if `inMemory` is true, those are
   * ignored and `dbPath` reports `:memory:`.
   *
   * In-memory ledgers cannot be shared across processes — each process
   * gets its own independent `:memory:` database.
   */
  inMemory?: boolean;
}

/** Filter for ledger queries. */
export interface LedgerFilter {
  runId?: string;
  kind?: EventKind | EventKind[];
  sinceId?: string;
  limit?: number;
}

export interface AppendInput {
  kind: EventKind;
  runId: string;
  payload: unknown;
  parentEventId?: string;
  sensitivity?: Sensitivity;
}

/** Subscription callback. */
export type LedgerListener = (event: ActantEvent) => void;

/** Append-only hash-chained event ledger backed by SQLite. */
export class Ledger {
  readonly project: string;
  readonly dbPath: string;
  private db: DatabaseSync;
  private listeners: Set<LedgerListener> = new Set();

  constructor(opts: LedgerOptions) {
    this.project = opts.project;
    if (opts.inMemory) {
      this.dbPath = ":memory:";
      this.db = new DatabaseSync(":memory:");
    } else {
      const root = opts.storeDir ?? join(homedir(), ".actantdb");
      this.dbPath = opts.dbPath ?? join(root, opts.project, "events.sqlite");
      mkdirSync(dirname(this.dbPath), { recursive: true });
      this.db = new DatabaseSync(this.dbPath);
    }
    this.db.exec(SCHEMA);
  }

  close(): void {
    this.db.close();
  }

  /** Path the ledger persists to. */
  path(): string {
    return this.dbPath;
  }

  /** Append a new event; computes id, hashes, and chain link. */
  append(input: AppendInput): ActantEvent {
    const id = ulid();
    const createdAt = new Date().toISOString();
    const payloadCanon = canonicalJSON(input.payload);
    const payloadHash = sha256(payloadCanon);
    const prevHash = this.lastChainHashForRun(input.runId) ?? GENESIS_HASH;
    const chainHash = nextChainHash(prevHash, payloadHash);
    const event: ActantEvent = {
      id,
      kind: input.kind,
      project: this.project,
      run_id: input.runId,
      payload: input.payload,
      payload_hash: payloadHash,
      chain_hash: chainHash,
      sensitivity: input.sensitivity ?? "low",
      created_at: createdAt,
      ...(input.parentEventId !== undefined ? { parent_event_id: input.parentEventId } : {}),
    };
    const stmt = this.db.prepare(
      `INSERT INTO events (
        id, kind, project, run_id, parent_event_id, payload, payload_hash,
        chain_hash, sensitivity, created_at
      ) VALUES (?,?,?,?,?,?,?,?,?,?)`,
    );
    stmt.run(
      event.id,
      event.kind,
      event.project,
      event.run_id,
      event.parent_event_id ?? null,
      payloadCanon,
      event.payload_hash,
      event.chain_hash,
      event.sensitivity,
      event.created_at,
    );
    for (const cb of this.listeners) {
      try {
        cb(event);
      } catch {
        // listeners are best-effort
      }
    }
    return event;
  }

  /** Read events matching the filter, in causal (id) order. */
  query(filter: LedgerFilter = {}): ActantEvent[] {
    const where: string[] = [];
    const params: unknown[] = [];
    if (filter.runId) {
      where.push("run_id = ?");
      params.push(filter.runId);
    }
    if (filter.kind) {
      const kinds = Array.isArray(filter.kind) ? filter.kind : [filter.kind];
      where.push(`kind IN (${kinds.map(() => "?").join(",")})`);
      for (const k of kinds) params.push(k);
    }
    if (filter.sinceId) {
      where.push("id > ?");
      params.push(filter.sinceId);
    }
    const wh = where.length ? "WHERE " + where.join(" AND ") : "";
    const limit = filter.limit ? `LIMIT ${Number(filter.limit)}` : "";
    const stmt = this.db.prepare(
      `SELECT id, kind, project, run_id, parent_event_id, payload, payload_hash,
              chain_hash, sensitivity, created_at
       FROM events ${wh} ORDER BY id ASC ${limit}`,
    );
    const rows = stmt.all(...(params as never[])) as unknown as RawRow[];
    return rows.map(rowToEvent);
  }

  /** Subscribe to new events. Returns an unsubscribe function. */
  subscribe(listener: LedgerListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Fetch a single event by id. */
  get(id: string): ActantEvent | undefined {
    const row = this.db
      .prepare(
        `SELECT id, kind, project, run_id, parent_event_id, payload, payload_hash,
                chain_hash, sensitivity, created_at FROM events WHERE id = ?`,
      )
      .get(id) as unknown as RawRow | undefined;
    return row ? rowToEvent(row) : undefined;
  }

  /** Build a checkpoint that captures everything needed to re-run from `eventId`. */
  checkpoint(eventId: string): CheckpointRef {
    const target = this.get(eventId);
    if (!target) throw new Error(`event not found: ${eventId}`);
    const prior = this.query({ runId: target.run_id, limit: 10_000 }).filter(
      (e) => e.id <= eventId,
    );
    // Latest manifest at-or-before this event
    const lastManifest = [...prior]
      .reverse()
      .find((e) => e.kind === "context_build");
    const manifestHash =
      (lastManifest?.payload as { manifest_hash?: string } | undefined)?.manifest_hash ?? "";
    // Latest guard verdict carries the policy snapshot
    const lastVerdict = [...prior].reverse().find((e) => e.kind === "guard_verdict");
    const policyHash =
      (lastVerdict?.payload as { policy_snapshot?: string } | undefined)?.policy_snapshot ?? "";
    // Memory ids in scope = ids of memory items included in lastManifest
    const memoryIds: string[] =
      (lastManifest?.payload as { included?: { id: string; kind: string }[] } | undefined)
        ?.included?.filter((i) => i.kind === "memory")
        .map((i) => i.id) ?? [];
    const memorySetHash = sha256OfJSON(memoryIds);
    const priorToolResults = prior
      .filter((e) => e.kind === "tool_call_completed")
      .map((e) => (e.payload as { tool_call_id: string }).tool_call_id);
    return {
      event_id: eventId,
      run_id: target.run_id,
      manifest_hash: manifestHash,
      policy_hash: policyHash,
      memory_set_hash: memorySetHash,
      prior_tool_results: priorToolResults,
    };
  }

  private lastChainHashForRun(runId: string): string | null {
    const row = this.db
      .prepare("SELECT chain_hash FROM events WHERE run_id = ? ORDER BY id DESC LIMIT 1")
      .get(runId) as unknown as { chain_hash?: string } | undefined;
    return row?.chain_hash ?? null;
  }
}

interface RawRow {
  id: string;
  kind: string;
  project: string;
  run_id: string;
  parent_event_id: string | null;
  payload: string;
  payload_hash: string;
  chain_hash: string;
  sensitivity: string;
  created_at: string;
}

function rowToEvent(r: RawRow): ActantEvent {
  return {
    id: r.id,
    kind: r.kind as EventKind,
    project: r.project,
    run_id: r.run_id,
    payload: JSON.parse(r.payload),
    payload_hash: r.payload_hash,
    chain_hash: r.chain_hash,
    sensitivity: r.sensitivity as Sensitivity,
    created_at: r.created_at,
    ...(r.parent_event_id ? { parent_event_id: r.parent_event_id } : {}),
  };
}

const SCHEMA = `
CREATE TABLE IF NOT EXISTS events (
  id              TEXT PRIMARY KEY,
  kind            TEXT NOT NULL,
  project         TEXT NOT NULL,
  run_id          TEXT NOT NULL,
  parent_event_id TEXT,
  payload         TEXT NOT NULL,
  payload_hash    TEXT NOT NULL,
  chain_hash      TEXT NOT NULL,
  sensitivity     TEXT NOT NULL,
  created_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_run ON events (run_id, id);
CREATE INDEX IF NOT EXISTS idx_events_kind ON events (kind, id);

CREATE TABLE IF NOT EXISTS approvals (
  tool_call_id TEXT PRIMARY KEY,
  run_id       TEXT NOT NULL,
  status       TEXT NOT NULL,
  request      TEXT NOT NULL,
  decision     TEXT,
  created_at   TEXT NOT NULL,
  decided_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_approvals_status ON approvals (status, created_at);

CREATE TABLE IF NOT EXISTS replay_runs (
  id           TEXT PRIMARY KEY,
  from_event   TEXT NOT NULL,
  original_run TEXT NOT NULL,
  overrides    TEXT NOT NULL,
  created_at   TEXT NOT NULL,
  events       TEXT NOT NULL
);
`;

/** Convenience: open the default ledger location for a project.
 *
 * Accepts either a positional `(project, storeDir?)` pair or the same
 * options object the `Ledger` constructor accepts. Object form is
 * preferred — it parallels the constructor and is easier to extend.
 */
export function openLedger(opts: LedgerOptions): Ledger;
export function openLedger(project: string, storeDir?: string): Ledger;
export function openLedger(
  arg: string | LedgerOptions,
  storeDir?: string,
): Ledger {
  if (typeof arg === "string") {
    return new Ledger({ project: arg, ...(storeDir !== undefined ? { storeDir } : {}) });
  }
  return new Ledger(arg);
}

/** Check whether a local ledger exists for a project. */
export function ledgerExists(project: string, storeDir?: string): boolean {
  const root = storeDir ?? join(homedir(), ".actantdb");
  return existsSync(join(root, project, "events.sqlite"));
}
