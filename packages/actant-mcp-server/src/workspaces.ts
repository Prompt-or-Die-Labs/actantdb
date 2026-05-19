/**
 * Workspace -> Ledger registry.
 *
 * MCP clients address ledger data by `workspace_id` and `session_id`.
 * The mapping into the embedded TS API is:
 *
 *   workspace_id -> project       (one SQLite file per workspace)
 *   session_id   -> run_id        (one row per session, scoped to workspace)
 *
 * Each workspace opens lazily on first use and is reused across tool calls
 * to avoid re-opening SQLite handles per request.
 */

import { openLedger, type Ledger } from "@actantdb/core";

export interface WorkspaceRegistryOptions {
  /** Root directory under which workspace ledgers live. */
  storeDir?: string;
  /**
   * Force every workspace to share a single in-memory ledger (used by tests
   * and by the smoke harness that constructs a ledger up front).
   */
  sharedLedger?: Ledger;
}

export class WorkspaceRegistry {
  private readonly storeDir: string | undefined;
  private readonly sharedLedger: Ledger | undefined;
  private readonly cache = new Map<string, Ledger>();

  constructor(opts: WorkspaceRegistryOptions = {}) {
    this.storeDir = opts.storeDir;
    this.sharedLedger = opts.sharedLedger;
  }

  get(workspaceId: string): Ledger {
    if (this.sharedLedger) return this.sharedLedger;
    const existing = this.cache.get(workspaceId);
    if (existing) return existing;
    const ledger = openLedger({
      project: workspaceId,
      ...(this.storeDir !== undefined ? { storeDir: this.storeDir } : {}),
    });
    this.cache.set(workspaceId, ledger);
    return ledger;
  }

  closeAll(): void {
    if (this.sharedLedger) return;
    for (const l of this.cache.values()) {
      try {
        l.close();
      } catch {
        // best-effort
      }
    }
    this.cache.clear();
  }
}
