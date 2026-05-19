/**
 * @actantdb/box — files namespace.
 *
 * Every operation records a ledger entry as:
 *   - tool_call_requested  (tool: "files.write" | "files.read" | ...)
 *   - tool_call_started
 *   - tool_call_completed  (status: "ok" | "error")
 *   - effect_observed      (payload.kind: "file_write" | "file_read" | ...)
 *
 * No new EventKinds — we layer "file_*" onto effect_observed.
 */

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";

import { ulid, type Ledger } from "@actantdb/core";

import { BoxError } from "./errors.js";
import {
  ensureParentDir,
  isDir,
  resolveInsideWorkspace,
  stripWorkspace,
} from "./box.js";
import type { FileEntry } from "./types.js";

export interface FilesCtx {
  ledger: Ledger;
  workspaceDir: string;
  cwd: string;
}

type CtxProvider = () => FilesCtx;

export class BoxFilesAPI {
  constructor(private readonly ctx: CtxProvider) {}

  async write(input: { path: string; content: string | Uint8Array }): Promise<void> {
    const { ledger, workspaceDir, cwd } = this.ctx();
    const abs = resolveInsideWorkspace(workspaceDir, cwd, input.path);
    const runId = `files-${ulid()}`;
    const toolCallId = ulid();
    const t0 = performance.now();

    ledger.append({
      kind: "tool_call_requested",
      runId,
      payload: {
        tool_call_id: toolCallId,
        tool: "files.write",
        args: { path: input.path, size: byteLength(input.content) },
        risk: "low",
      },
      sensitivity: "low",
    });
    ledger.append({
      kind: "tool_call_started",
      runId,
      payload: { tool_call_id: toolCallId, final_args: { path: input.path } },
      sensitivity: "low",
    });

    try {
      ensureParentDir(abs);
      writeFileSync(abs, input.content);
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status: "ok",
          result: { path: input.path, bytes: byteLength(input.content) },
          duration_ms: Math.round(performance.now() - t0),
        },
        sensitivity: "low",
      });
      ledger.append({
        kind: "effect_observed",
        runId,
        payload: {
          kind: "file_write",
          path: stripWorkspace(workspaceDir, abs),
          bytes: byteLength(input.content),
        },
        sensitivity: "low",
      });
    } catch (err) {
      recordToolError(ledger, runId, toolCallId, t0, err);
      throw new BoxError(
        "io_error",
        `files.write failed: ${(err as Error).message}`,
        err,
      );
    }
  }

  async read(path: string): Promise<string> {
    const { ledger, workspaceDir, cwd } = this.ctx();
    const abs = resolveInsideWorkspace(workspaceDir, cwd, path);
    const runId = `files-${ulid()}`;
    const toolCallId = ulid();
    const t0 = performance.now();

    ledger.append({
      kind: "tool_call_requested",
      runId,
      payload: {
        tool_call_id: toolCallId,
        tool: "files.read",
        args: { path },
        risk: "low",
      },
      sensitivity: "low",
    });
    ledger.append({
      kind: "tool_call_started",
      runId,
      payload: { tool_call_id: toolCallId, final_args: { path } },
      sensitivity: "low",
    });

    try {
      if (!existsSync(abs)) {
        throw new BoxError("not_found", `files.read: ${path} does not exist`);
      }
      const content = readFileSync(abs, "utf8");
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status: "ok",
          result: { path, bytes: byteLength(content) },
          duration_ms: Math.round(performance.now() - t0),
        },
        sensitivity: "low",
      });
      ledger.append({
        kind: "effect_observed",
        runId,
        payload: {
          kind: "file_read",
          path: stripWorkspace(workspaceDir, abs),
          bytes: byteLength(content),
        },
        sensitivity: "low",
      });
      return content;
    } catch (err) {
      recordToolError(ledger, runId, toolCallId, t0, err);
      if (err instanceof BoxError) throw err;
      throw new BoxError(
        "io_error",
        `files.read failed: ${(err as Error).message}`,
        err,
      );
    }
  }

  async list(dir = "."): Promise<FileEntry[]> {
    const { ledger, workspaceDir, cwd } = this.ctx();
    const abs = resolveInsideWorkspace(workspaceDir, cwd, dir);
    const runId = `files-${ulid()}`;
    const toolCallId = ulid();
    const t0 = performance.now();

    ledger.append({
      kind: "tool_call_requested",
      runId,
      payload: {
        tool_call_id: toolCallId,
        tool: "files.list",
        args: { dir },
        risk: "low",
      },
      sensitivity: "low",
    });
    ledger.append({
      kind: "tool_call_started",
      runId,
      payload: { tool_call_id: toolCallId, final_args: { dir } },
      sensitivity: "low",
    });

    try {
      if (!existsSync(abs)) {
        throw new BoxError("not_found", `files.list: ${dir} does not exist`);
      }
      const entries = readdirSync(abs, { withFileTypes: true });
      const out: FileEntry[] = entries.map((e) => {
        const entryAbs = join(abs, e.name);
        let size = 0;
        try {
          size = statSync(entryAbs).size;
        } catch {
          /* ignore */
        }
        return {
          path: stripWorkspace(workspaceDir, entryAbs),
          absolutePath: entryAbs,
          kind: e.isDirectory() ? "directory" : e.isFile() ? "file" : "other",
          size: e.isDirectory() ? 0 : size,
        };
      });
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status: "ok",
          result: { count: out.length },
          duration_ms: Math.round(performance.now() - t0),
        },
        sensitivity: "low",
      });
      return out;
    } catch (err) {
      recordToolError(ledger, runId, toolCallId, t0, err);
      if (err instanceof BoxError) throw err;
      throw new BoxError(
        "io_error",
        `files.list failed: ${(err as Error).message}`,
        err,
      );
    }
  }

  async upload(items: { path: string; destination: string }[]): Promise<void> {
    const { ledger, workspaceDir, cwd } = this.ctx();
    const runId = `files-${ulid()}`;
    const toolCallId = ulid();
    const t0 = performance.now();
    ledger.append({
      kind: "tool_call_requested",
      runId,
      payload: {
        tool_call_id: toolCallId,
        tool: "files.upload",
        args: { count: items.length },
        risk: "low",
      },
      sensitivity: "low",
    });
    ledger.append({
      kind: "tool_call_started",
      runId,
      payload: { tool_call_id: toolCallId, final_args: { count: items.length } },
      sensitivity: "low",
    });
    try {
      for (const item of items) {
        const src = isAbsolute(item.path) ? item.path : resolve(process.cwd(), item.path);
        if (!existsSync(src)) {
          throw new BoxError("not_found", `files.upload: source ${item.path} missing`);
        }
        const destAbs = resolveInsideWorkspace(workspaceDir, cwd, item.destination);
        ensureParentDir(destAbs);
        copyFileSync(src, destAbs);
        ledger.append({
          kind: "effect_observed",
          runId,
          payload: {
            kind: "file_upload",
            src: item.path,
            dest: stripWorkspace(workspaceDir, destAbs),
          },
          sensitivity: "low",
        });
      }
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status: "ok",
          result: { count: items.length },
          duration_ms: Math.round(performance.now() - t0),
        },
        sensitivity: "low",
      });
    } catch (err) {
      recordToolError(ledger, runId, toolCallId, t0, err);
      if (err instanceof BoxError) throw err;
      throw new BoxError(
        "io_error",
        `files.upload failed: ${(err as Error).message}`,
        err,
      );
    }
  }

  async download(opts: { folder: string }): Promise<void> {
    const { ledger, workspaceDir, cwd } = this.ctx();
    const runId = `files-${ulid()}`;
    const toolCallId = ulid();
    const t0 = performance.now();
    ledger.append({
      kind: "tool_call_requested",
      runId,
      payload: {
        tool_call_id: toolCallId,
        tool: "files.download",
        args: { folder: opts.folder },
        risk: "low",
      },
      sensitivity: "low",
    });
    ledger.append({
      kind: "tool_call_started",
      runId,
      payload: { tool_call_id: toolCallId, final_args: { folder: opts.folder } },
      sensitivity: "low",
    });
    try {
      const target = isAbsolute(opts.folder)
        ? opts.folder
        : resolve(process.cwd(), opts.folder);
      mkdirSync(target, { recursive: true });
      const sourceRoot = cwd
        ? resolveInsideWorkspace(workspaceDir, "", cwd)
        : workspaceDir;
      const copied = copyTreeSync(sourceRoot, target);
      ledger.append({
        kind: "tool_call_completed",
        runId,
        payload: {
          tool_call_id: toolCallId,
          status: "ok",
          result: { copied },
          duration_ms: Math.round(performance.now() - t0),
        },
        sensitivity: "low",
      });
      ledger.append({
        kind: "effect_observed",
        runId,
        payload: { kind: "file_download", folder: opts.folder, copied },
        sensitivity: "low",
      });
    } catch (err) {
      recordToolError(ledger, runId, toolCallId, t0, err);
      throw new BoxError(
        "io_error",
        `files.download failed: ${(err as Error).message}`,
        err,
      );
    }
  }
}

// ----- helpers -----

function byteLength(content: string | Uint8Array): number {
  return typeof content === "string" ? Buffer.byteLength(content, "utf8") : content.byteLength;
}

function recordToolError(
  ledger: Ledger,
  runId: string,
  toolCallId: string,
  t0: number,
  err: unknown,
): void {
  ledger.append({
    kind: "tool_call_completed",
    runId,
    payload: {
      tool_call_id: toolCallId,
      status: "error",
      result: { error: (err as Error).message ?? String(err) },
      duration_ms: Math.round(performance.now() - t0),
    },
    sensitivity: "low",
  });
}

/** Minimal recursive copy. Returns number of files copied. */
function copyTreeSync(src: string, dest: string): number {
  let count = 0;
  if (!isDir(src)) {
    if (existsSync(src)) {
      ensureParentDir(dest);
      copyFileSync(src, dest);
      return 1;
    }
    return 0;
  }
  mkdirSync(dest, { recursive: true });
  for (const ent of readdirSync(src, { withFileTypes: true })) {
    const s = join(src, ent.name);
    const d = join(dest, ent.name);
    if (ent.isDirectory()) {
      // Skip the embedded .actantdb dir on download — that's box-private state.
      if (basename(s) === ".actantdb") continue;
      count += copyTreeSync(s, d);
    } else if (ent.isFile()) {
      ensureParentDir(d);
      copyFileSync(s, d);
      count++;
    }
  }
  // suppress unused dirname warning
  void dirname;
  return count;
}
