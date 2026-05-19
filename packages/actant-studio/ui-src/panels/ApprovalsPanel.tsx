import * as React from "react";
import { useCallback, useEffect, useState } from "react";

import { api, type ApprovalRecord } from "../lib/api.js";

const POLL_MS = 3000;

interface ApprovalsPanelProps {
  onDecide: (toolCallId: string, kind: "approve" | "approve_constrained" | "deny") => void;
}

export function ApprovalsPanel({ onDecide }: ApprovalsPanelProps): React.JSX.Element | null {
  const [pending, setPending] = useState<ApprovalRecord[]>([]);

  const refresh = useCallback(async () => {
    try {
      const r = await api.approvals();
      setPending((r.approvals ?? []).filter((a) => a.status === "pending"));
    } catch {
      // surfaced in App via /api/info polling; quiet here.
    }
  }, []);

  useEffect(() => {
    refresh();
    const id = window.setInterval(refresh, POLL_MS);
    return () => window.clearInterval(id);
  }, [refresh]);

  if (pending.length === 0) {
    return null;
  }

  return (
    <div className="approvals">
      <h3>Pending approvals · {pending.length}</h3>
      {pending.map((a) => {
        const hasConstrained = a.request.constrained_input !== undefined;
        return (
          <div className="row" key={a.toolCallId}>
            <div className="info" title={a.toolCallId}>
              {a.request.tool} · {JSON.stringify(a.request.args)}
              {a.request.hint ? ` · hint: ${String(a.request.hint)}` : ""}
            </div>
            <div className="actions">
              <button
                onClick={() => {
                  onDecide(a.toolCallId, "approve");
                  refresh();
                }}
              >
                Approve
              </button>
              {hasConstrained && (
                <button
                  className="secondary"
                  onClick={() => {
                    onDecide(a.toolCallId, "approve_constrained");
                    refresh();
                  }}
                >
                  Approve constrained
                </button>
              )}
              <button
                className="danger"
                onClick={() => {
                  onDecide(a.toolCallId, "deny");
                  refresh();
                }}
              >
                Deny
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
