/**
 * @actantdb/box — snapshot / restore.
 *
 * Implementation note: rather than pulling in a tar dep, we snapshot the
 * workspace as a directory copy under
 *
 *   <storeRoot>/.snapshots/<snapshot_id>/
 *     workspace/...    (a deep copy of the box's workspace dir)
 *     snapshot.json    (Snapshot metadata, including anchor event id)
 *
 * `archivePath` in the returned Snapshot points at the snapshot dir. Cloud
 * mode will eventually swap this for a real tar uploaded to object storage.
 */

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join } from "node:path";

import { ulid } from "@actantdb/core";

import { BoxError } from "./errors.js";
import type { Snapshot } from "./types.js";

export function snapshotsRoot(storeRoot: string): string {
  return join(storeRoot, ".snapshots");
}

export async function snapshotBox(opts: {
  boxId: string;
  workspaceDir: string;
  storeRoot: string;
  name?: string;
  anchorEventId?: string;
}): Promise<Snapshot> {
  const id = `snap-${ulid().toLowerCase()}`;
  const root = snapshotsRoot(opts.storeRoot);
  const snapDir = join(root, id);
  const wsCopy = join(snapDir, "workspace");
  mkdirSync(wsCopy, { recursive: true });

  copyTreeSync(opts.workspaceDir, wsCopy);

  const snap: Snapshot = {
    id,
    boxId: opts.boxId,
    createdAt: new Date().toISOString(),
    archivePath: snapDir,
    ...(opts.name !== undefined ? { name: opts.name } : {}),
    ...(opts.anchorEventId !== undefined ? { anchorEventId: opts.anchorEventId } : {}),
  };
  writeFileSync(join(snapDir, "snapshot.json"), JSON.stringify(snap, null, 2), "utf8");
  return snap;
}

export async function fromSnapshot(opts: {
  snapshotId: string;
  workspaceDir: string;
  storeRoot: string;
}): Promise<void> {
  const snapDir = join(snapshotsRoot(opts.storeRoot), opts.snapshotId);
  const metaFile = join(snapDir, "snapshot.json");
  if (!existsSync(metaFile)) {
    throw new BoxError(
      "snapshot_not_found",
      `snapshot ${opts.snapshotId} missing at ${snapDir}`,
    );
  }
  const wsCopy = join(snapDir, "workspace");
  if (!existsSync(wsCopy)) {
    throw new BoxError(
      "snapshot_not_found",
      `snapshot ${opts.snapshotId} payload missing (no workspace dir)`,
    );
  }
  copyTreeSync(wsCopy, opts.workspaceDir);
}

export async function listSnapshotsForBox(opts: {
  boxId: string;
  storeRoot: string;
}): Promise<Snapshot[]> {
  const root = snapshotsRoot(opts.storeRoot);
  if (!existsSync(root)) return [];
  const entries = readdirSync(root, { withFileTypes: true });
  const out: Snapshot[] = [];
  for (const e of entries) {
    if (!e.isDirectory()) continue;
    const metaFile = join(root, e.name, "snapshot.json");
    if (!existsSync(metaFile)) continue;
    try {
      const meta = JSON.parse(readFileSync(metaFile, "utf8")) as Snapshot;
      if (meta.boxId === opts.boxId) out.push(meta);
    } catch {
      /* skip corrupt */
    }
  }
  return out.sort((a, b) => a.createdAt.localeCompare(b.createdAt));
}

export async function deleteSnapshot(opts: {
  id: string;
  storeRoot: string;
}): Promise<void> {
  const snapDir = join(snapshotsRoot(opts.storeRoot), opts.id);
  if (!existsSync(snapDir)) {
    throw new BoxError("snapshot_not_found", `snapshot ${opts.id} not found`);
  }
  rmSync(snapDir, { recursive: true, force: true });
}

// ----- helpers -----

function copyTreeSync(src: string, dest: string): void {
  if (!existsSync(src)) return;
  const stat = statSync(src);
  if (!stat.isDirectory()) {
    mkdirSync(dirname(dest), { recursive: true });
    copyFileSync(src, dest);
    return;
  }
  mkdirSync(dest, { recursive: true });
  for (const ent of readdirSync(src, { withFileTypes: true })) {
    const s = join(src, ent.name);
    const d = join(dest, ent.name);
    if (ent.isDirectory()) {
      // Skip the per-box ledger dir; snapshots store workspace content only.
      // A fresh ledger is opened on `Box.fromSnapshot`.
      if (basename(s) === ".actantdb") continue;
      copyTreeSync(s, d);
    } else if (ent.isFile()) {
      copyFileSync(s, d);
    }
  }
}
