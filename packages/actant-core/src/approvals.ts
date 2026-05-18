import type { Ledger } from "./ledger.js";
import type { ApprovalRequest, ApprovalDecision } from "@actantdb/types";

export interface ApprovalRecord {
  toolCallId: string;
  runId: string;
  status: "pending" | "approved" | "approved_constrained" | "denied";
  request: ApprovalRequest;
  decision?: ApprovalDecision;
  createdAt: string;
  decidedAt?: string;
}

export class ApprovalStore {
  constructor(private ledger: Ledger) {}

  private db(): DatabaseLike {
    // Reach the inner DB via a private channel.
    return (this.ledger as unknown as { db: DatabaseLike }).db;
  }

  /** Record a pending approval. */
  enqueue(req: ApprovalRequest, runId: string): void {
    const now = new Date().toISOString();
    const stmt = this.db().prepare(
      `INSERT OR REPLACE INTO approvals (tool_call_id, run_id, status, request, created_at)
       VALUES (?,?,?,?,?)`,
    );
    stmt.run(req.tool_call_id, runId, "pending", JSON.stringify(req), now);
  }

  /** Mark an approval decided. Idempotent for the same tool_call_id. */
  decide(toolCallId: string, decision: ApprovalDecision): ApprovalRecord {
    const now = new Date().toISOString();
    const status =
      decision.decision === "approve"
        ? "approved"
        : decision.decision === "approve_constrained"
          ? "approved_constrained"
          : "denied";
    const stmt = this.db().prepare(
      `UPDATE approvals SET status = ?, decision = ?, decided_at = ? WHERE tool_call_id = ?`,
    );
    stmt.run(status, JSON.stringify(decision), now, toolCallId);
    const rec = this.get(toolCallId);
    if (!rec) throw new Error(`approval not found: ${toolCallId}`);
    return rec;
  }

  pending(): ApprovalRecord[] {
    const rows = this.db()
      .prepare(`SELECT * FROM approvals WHERE status = 'pending' ORDER BY created_at ASC`)
      .all() as RawApprovalRow[];
    return rows.map(rowToRecord);
  }

  all(): ApprovalRecord[] {
    const rows = this.db()
      .prepare(`SELECT * FROM approvals ORDER BY created_at ASC`)
      .all() as RawApprovalRow[];
    return rows.map(rowToRecord);
  }

  get(toolCallId: string): ApprovalRecord | undefined {
    const row = this.db()
      .prepare(`SELECT * FROM approvals WHERE tool_call_id = ?`)
      .get(toolCallId) as RawApprovalRow | undefined;
    return row ? rowToRecord(row) : undefined;
  }
}

interface DatabaseLike {
  prepare(sql: string): {
    run(...p: unknown[]): unknown;
    get(...p: unknown[]): unknown;
    all(...p: unknown[]): unknown[];
  };
}

interface RawApprovalRow {
  tool_call_id: string;
  run_id: string;
  status: string;
  request: string;
  decision: string | null;
  created_at: string;
  decided_at: string | null;
}

function rowToRecord(r: RawApprovalRow): ApprovalRecord {
  return {
    toolCallId: r.tool_call_id,
    runId: r.run_id,
    status: r.status as ApprovalRecord["status"],
    request: JSON.parse(r.request) as ApprovalRequest,
    createdAt: r.created_at,
    ...(r.decision ? { decision: JSON.parse(r.decision) as ApprovalDecision } : {}),
    ...(r.decided_at ? { decidedAt: r.decided_at } : {}),
  };
}
