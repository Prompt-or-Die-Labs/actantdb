/**
 * @actantdb/box — Box class.
 *
 * A Box owns:
 *   - a workspace directory on disk (~/.actantdb/boxes/<id>/workspace)
 *   - a per-box ledger (~/.actantdb/boxes/<id>/.actantdb/events.sqlite)
 *   - a `box.json` metadata file (~/.actantdb/boxes/<id>/box.json)
 *   - five subsystem namespaces: agent, exec, files, git, schedule.
 *
 * Lifecycle:
 *   Box.create()    → mkdir workspace + open ledger + write box.json
 *   Box.get(id)     → read box.json + reopen ledger
 *   Box.getByName() → scan box.json files, match name
 *   Box.list()      → scan box.json files, return metadata
 *   Box.fromSnapshot(snapshotId) → untar snapshot into a fresh box
 *   box.pause()/resume()/delete() → flip status, optionally rm -rf
 *
 * Ledger reuse:
 *   `withActant` (used by box.agent) internally opens its own ledger by
 *   (project, storeDir). To stay on the same SQLite file we share the
 *   project name and storeRoot. SQLite tolerates two handles on one file
 *   (WAL or otherwise) but we close them in order on `box.delete`.
 */

import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, isAbsolute, join, resolve } from "node:path";

import {
  openLedger,
  ulid,
  type Ledger,
} from "@actantdb/core";

import { BoxAgentAPI } from "./agent.js";
import { cloudNotImplemented } from "./cloud.js";
import { BoxError } from "./errors.js";
import { BoxExecAPI } from "./exec.js";
import { BoxFilesAPI } from "./files.js";
import { BoxGitAPI } from "./git.js";
import { BoxScheduleAPI } from "./schedule.js";
import {
  fromSnapshot,
  listSnapshotsForBox,
  snapshotBox,
  deleteSnapshot as deleteSnapshotImpl,
} from "./snapshot.js";
import type {
  BoxConfig,
  BoxData,
  BoxMetadata,
  BoxStatus,
  Snapshot,
} from "./types.js";

const BOX_JSON = "box.json";
const LEDGER_DIRNAME = ".actantdb";
const WORKSPACE_DIRNAME = "workspace";

/** Default root: ~/.actantdb/boxes */
export function defaultBoxesRoot(): string {
  return join(homedir(), ".actantdb", "boxes");
}

export class Box {
  readonly id: string;
  readonly name: string;
  readonly mode: "local" | "cloud";
  readonly workspaceDir: string;
  readonly storeRoot: string;
  readonly project: string;
  readonly createdAt: string;
  /** Underlying ledger. Studio uses this directly. */
  readonly ledger: Ledger;

  /** Subsystem APIs. */
  readonly agent: BoxAgentAPI;
  readonly exec: BoxExecAPI;
  readonly files: BoxFilesAPI;
  readonly git: BoxGitAPI;
  readonly schedule: BoxScheduleAPI;

  // ----- mutable state -----
  private _cwd: string;
  private _model: string | undefined;
  private _initCommand: string | undefined;
  private _status: BoxStatus;
  private _keepAlive: boolean;
  private _agentRef: BoxAgentAPI["agent"]; // kept stable for box.agent

  private constructor(args: {
    metadata: BoxMetadata;
    ledger: Ledger;
    config: BoxConfig;
  }) {
    const { metadata, ledger, config } = args;
    this.id = metadata.id;
    this.name = metadata.name;
    this.mode = metadata.mode;
    this.workspaceDir = metadata.workspaceDir;
    this.storeRoot = metadata.storeRoot;
    this.project = metadata.project;
    this.createdAt = metadata.createdAt;
    this.ledger = ledger;
    this._cwd = metadata.cwd;
    this._model = metadata.model;
    this._initCommand = metadata.initCommand;
    this._status = metadata.status;
    this._keepAlive = metadata.keepAlive;

    const ctxProvider = () => ({
      ledger: this.ledger,
      workspaceDir: this.workspaceDir,
      cwd: this._cwd,
      project: this.project,
      storeRoot: this.storeRoot,
      ledgerStoreDir: join(this.storeRoot, this.id, LEDGER_DIRNAME),
      mode: this.mode,
    });

    this.agent = new BoxAgentAPI(ctxProvider, config.agent);
    this.exec = new BoxExecAPI(ctxProvider);
    this.files = new BoxFilesAPI(ctxProvider);
    this.git = new BoxGitAPI(ctxProvider);
    this.schedule = new BoxScheduleAPI(ctxProvider, () => this.exec, () => this.agent);
    this._agentRef = this.agent.agent;
  }

  // ============================================================
  // Static API
  // ============================================================

  static async create(config: BoxConfig = {}): Promise<Box> {
    const mode = config.mode ?? "local";
    const id = ulid().toLowerCase();
    const name = config.name ?? id;
    const storeRoot = config.storeRoot ?? defaultBoxesRoot();
    const boxRoot = join(storeRoot, id);
    const workspaceDir = join(boxRoot, WORKSPACE_DIRNAME);
    const ledgerStoreDir = join(boxRoot, LEDGER_DIRNAME);
    const project = config.project ?? `box-${id}`;

    if (mode === "cloud") {
      // The contract is in place but every operation throws.
      const metadata: BoxMetadata = {
        id,
        name,
        createdAt: new Date().toISOString(),
        mode,
        storeRoot,
        workspaceDir,
        cwd: config.cwd ?? "",
        project,
        ...(config.model !== undefined ? { model: config.model } : {}),
        ...(config.initCommand !== undefined ? { initCommand: config.initCommand } : {}),
        keepAlive: config.keepAlive ?? true,
        status: "running",
      };
      const ledger = openLedger({ project, inMemory: true });
      return new Box({ metadata, ledger, config });
    }

    mkdirSync(workspaceDir, { recursive: true });
    mkdirSync(ledgerStoreDir, { recursive: true });

    const metadata: BoxMetadata = {
      id,
      name,
      createdAt: new Date().toISOString(),
      mode,
      storeRoot,
      workspaceDir,
      cwd: config.cwd ?? "",
      project,
      ...(config.model !== undefined ? { model: config.model } : {}),
      ...(config.initCommand !== undefined ? { initCommand: config.initCommand } : {}),
      keepAlive: config.keepAlive ?? true,
      status: "running",
    };
    writeMetadata(boxRoot, metadata);

    // Open the per-box ledger. We point storeDir at the box root so the
    // file ends up at <boxRoot>/.actantdb/<project>/events.sqlite.
    const ledger = openLedger({ project, storeDir: ledgerStoreDir });

    const box = new Box({ metadata, ledger, config });

    // Record a box_created event so the timeline starts at the start.
    ledger.append({
      kind: "effect_observed",
      runId: `box-${id}`,
      payload: { kind: "box_created", box_id: id, name, workspaceDir },
      sensitivity: "low",
    });

    if (config.initCommand) {
      try {
        await box.exec.command(config.initCommand);
      } catch (err) {
        // Init failure is recorded as an effect but does not nuke the box —
        // the user can re-run with `box.setInitCommand(...)` then `box.exec.command(...)`.
        ledger.append({
          kind: "effect_observed",
          runId: `box-${id}`,
          payload: {
            kind: "box_init_failed",
            error: (err as Error).message ?? String(err),
          },
          sensitivity: "low",
        });
      }
    }

    return box;
  }

  static async get(boxId: string, opts: { storeRoot?: string } = {}): Promise<Box> {
    const storeRoot = opts.storeRoot ?? defaultBoxesRoot();
    const boxRoot = join(storeRoot, boxId);
    const metaPath = join(boxRoot, BOX_JSON);
    if (!existsSync(metaPath)) {
      throw new BoxError("not_found", `box ${boxId} not found at ${boxRoot}`);
    }
    const metadata = readMetadata(boxRoot);
    if (metadata.mode === "cloud") cloudNotImplemented("Box.get");
    const ledger = openLedger({
      project: metadata.project,
      storeDir: join(boxRoot, LEDGER_DIRNAME),
    });
    const box = new Box({ metadata, ledger, config: {} });
    await box.schedule.restore();
    return box;
  }

  static async getByName(name: string, opts: { storeRoot?: string } = {}): Promise<Box> {
    const boxes = await Box.list(opts);
    const match = boxes.find((b) => b.name === name);
    if (!match) throw new BoxError("not_found", `no box named ${name}`);
    return Box.get(match.id, opts);
  }

  static async list(opts: { storeRoot?: string } = {}): Promise<BoxData[]> {
    const storeRoot = opts.storeRoot ?? defaultBoxesRoot();
    if (!existsSync(storeRoot)) return [];
    const entries = readdirSync(storeRoot, { withFileTypes: true });
    const out: BoxData[] = [];
    for (const dirent of entries) {
      if (!dirent.isDirectory()) continue;
      const metaPath = join(storeRoot, dirent.name, BOX_JSON);
      if (!existsSync(metaPath)) continue;
      try {
        const meta = readMetadata(join(storeRoot, dirent.name));
        out.push({
          id: meta.id,
          name: meta.name,
          createdAt: meta.createdAt,
          workspaceDir: meta.workspaceDir,
          status: meta.status,
          mode: meta.mode,
          ...(meta.model !== undefined ? { model: meta.model } : {}),
        });
      } catch {
        // skip unreadable box.json
      }
    }
    return out.sort((a, b) => a.createdAt.localeCompare(b.createdAt));
  }

  static async fromSnapshot(
    snapshotId: string,
    config: BoxConfig = {},
  ): Promise<Box> {
    const mode = config.mode ?? "local";
    if (mode === "cloud") cloudNotImplemented("Box.fromSnapshot");
    const id = ulid().toLowerCase();
    const name = config.name ?? id;
    const storeRoot = config.storeRoot ?? defaultBoxesRoot();
    const boxRoot = join(storeRoot, id);
    const workspaceDir = join(boxRoot, WORKSPACE_DIRNAME);
    const ledgerStoreDir = join(boxRoot, LEDGER_DIRNAME);
    const project = config.project ?? `box-${id}`;

    mkdirSync(workspaceDir, { recursive: true });
    mkdirSync(ledgerStoreDir, { recursive: true });

    await fromSnapshot({
      snapshotId,
      workspaceDir,
      storeRoot,
    });

    const metadata: BoxMetadata = {
      id,
      name,
      createdAt: new Date().toISOString(),
      mode,
      storeRoot,
      workspaceDir,
      cwd: config.cwd ?? "",
      project,
      ...(config.model !== undefined ? { model: config.model } : {}),
      ...(config.initCommand !== undefined ? { initCommand: config.initCommand } : {}),
      keepAlive: config.keepAlive ?? true,
      status: "running",
    };
    writeMetadata(boxRoot, metadata);
    const ledger = openLedger({ project, storeDir: ledgerStoreDir });
    ledger.append({
      kind: "effect_observed",
      runId: `box-${id}`,
      payload: { kind: "box_restored_from_snapshot", snapshot_id: snapshotId },
      sensitivity: "low",
    });
    return new Box({ metadata, ledger, config });
  }

  // ============================================================
  // Instance API
  // ============================================================

  get cwd(): string {
    return this._cwd;
  }

  /** Same shape Upstash exposes: `{ harness, model }`. Harness is always "local". */
  get modelConfig(): { harness: string; model: string | undefined } {
    return { harness: this.mode === "cloud" ? "cloud" : "local", model: this._model };
  }

  get keepAlive(): boolean {
    return this._keepAlive;
  }

  set keepAlive(v: boolean) {
    this._keepAlive = v;
    this.persistMetadata();
  }

  async cd(path: string): Promise<void> {
    if (this.mode === "cloud") cloudNotImplemented("box.cd");
    // Resolve relative to the workspace dir; reject paths that escape it.
    const resolved = resolveInsideWorkspace(this.workspaceDir, this._cwd, path);
    const rel = stripWorkspace(this.workspaceDir, resolved);
    this._cwd = rel;
    this.persistMetadata();
  }

  async configureModel(model: string): Promise<void> {
    this._model = model;
    this.persistMetadata();
    this.ledger.append({
      kind: "effect_observed",
      runId: `box-${this.id}`,
      payload: { kind: "model_configured", model },
      sensitivity: "low",
    });
  }

  async getStatus(): Promise<{ status: BoxStatus }> {
    return { status: this._status };
  }

  // ----- init command -----
  async getInitCommand(): Promise<string | undefined> {
    return this._initCommand;
  }
  async setInitCommand(cmd: string): Promise<void> {
    this._initCommand = cmd;
    this.persistMetadata();
  }
  async deleteInitCommand(): Promise<void> {
    this._initCommand = undefined;
    this.persistMetadata();
  }

  async pause(): Promise<void> {
    if (this.mode === "cloud") cloudNotImplemented("box.pause");
    this._status = "paused";
    this.schedule.pauseAll();
    this.persistMetadata();
    this.ledger.append({
      kind: "effect_observed",
      runId: `box-${this.id}`,
      payload: { kind: "box_paused" },
      sensitivity: "low",
    });
  }

  async resume(): Promise<void> {
    if (this.mode === "cloud") cloudNotImplemented("box.resume");
    this._status = "running";
    this.schedule.resumeAll();
    this.persistMetadata();
    this.ledger.append({
      kind: "effect_observed",
      runId: `box-${this.id}`,
      payload: { kind: "box_resumed" },
      sensitivity: "low",
    });
  }

  async delete(): Promise<void> {
    if (this.mode === "cloud") cloudNotImplemented("box.delete");
    this._status = "deleted";
    this.schedule.stopAll();
    // Close the wrapper-owned ledger handle (if any) before closing ours.
    this.agent.close();
    try {
      this.ledger.close();
    } catch {
      /* ignore */
    }
    const boxRoot = join(this.storeRoot, this.id);
    try {
      rmSync(boxRoot, { recursive: true, force: true });
    } catch (err) {
      throw new BoxError(
        "io_error",
        `failed to delete box ${this.id}: ${(err as Error).message}`,
        err,
      );
    }
  }

  // ----- snapshots -----

  async snapshot(opts: { name?: string } = {}): Promise<Snapshot> {
    if (this.mode === "cloud") cloudNotImplemented("box.snapshot");
    const events = this.ledger.query({});
    const anchor = events[events.length - 1];
    const snap = await snapshotBox({
      boxId: this.id,
      workspaceDir: this.workspaceDir,
      storeRoot: this.storeRoot,
      ...(opts.name !== undefined ? { name: opts.name } : {}),
      ...(anchor ? { anchorEventId: anchor.id } : {}),
    });
    this.ledger.append({
      kind: "effect_observed",
      runId: `box-${this.id}`,
      payload: {
        kind: "snapshot_created",
        snapshot_id: snap.id,
        ...(opts.name !== undefined ? { name: opts.name } : {}),
      },
      sensitivity: "low",
    });
    return snap;
  }

  async listSnapshots(): Promise<Snapshot[]> {
    return listSnapshotsForBox({ boxId: this.id, storeRoot: this.storeRoot });
  }

  async deleteSnapshot(id: string): Promise<void> {
    await deleteSnapshotImpl({ id, storeRoot: this.storeRoot });
  }

  // ============================================================
  // Internal
  // ============================================================

  /** Persist box.json after mutable-state changes. */
  private persistMetadata(): void {
    if (this.mode === "cloud") return;
    const metadata: BoxMetadata = {
      id: this.id,
      name: this.name,
      createdAt: this.createdAt,
      mode: this.mode,
      storeRoot: this.storeRoot,
      workspaceDir: this.workspaceDir,
      cwd: this._cwd,
      project: this.project,
      ...(this._model !== undefined ? { model: this._model } : {}),
      ...(this._initCommand !== undefined ? { initCommand: this._initCommand } : {}),
      keepAlive: this._keepAlive,
      status: this._status,
    };
    writeMetadata(join(this.storeRoot, this.id), metadata);
  }
}

// ----- metadata helpers -----

function writeMetadata(boxRoot: string, meta: BoxMetadata): void {
  mkdirSync(boxRoot, { recursive: true });
  writeFileSync(join(boxRoot, BOX_JSON), JSON.stringify(meta, null, 2), "utf8");
}

function readMetadata(boxRoot: string): BoxMetadata {
  const p = join(boxRoot, BOX_JSON);
  if (!existsSync(p)) {
    throw new BoxError("not_found", `box.json missing at ${p}`);
  }
  const raw = readFileSync(p, "utf8");
  return JSON.parse(raw) as BoxMetadata;
}

/** Resolve a (possibly relative) path against the workspace + cwd, refusing
 *  to escape the workspace dir. Returns an absolute path. */
export function resolveInsideWorkspace(
  workspaceDir: string,
  cwd: string,
  path: string,
): string {
  const base = cwd ? join(workspaceDir, cwd) : workspaceDir;
  const target = isAbsolute(path) ? path : resolve(base, path);
  const wsResolved = resolve(workspaceDir);
  if (!target.startsWith(wsResolved)) {
    throw new BoxError(
      "invalid_argument",
      `path '${path}' resolves to '${target}' which escapes the workspace at '${wsResolved}'`,
    );
  }
  return target;
}

/** Strip the workspace dir prefix; returns a workspace-relative path. */
export function stripWorkspace(workspaceDir: string, absPath: string): string {
  const ws = resolve(workspaceDir);
  const rel = absPath.startsWith(ws) ? absPath.slice(ws.length) : absPath;
  return rel.replace(/^[/\\]+/, "");
}

/** Ensure parent dir exists; convenience for writeFileSync paths. */
export function ensureParentDir(target: string): void {
  mkdirSync(dirname(target), { recursive: true });
}

/** Best-effort: is `path` a directory? */
export function isDir(path: string): boolean {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
}
