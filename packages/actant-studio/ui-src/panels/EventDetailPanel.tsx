import * as React from "react";
import type { ActantEvent } from "@actantdb/types";

import type { ReplayResponse } from "../lib/api.js";
import { ReplayPanel } from "./ReplayPanel.js";

interface EventDetailPanelProps {
  event: ActantEvent | null;
  replayResult: ReplayResponse | null;
  onReplay: (r: ReplayResponse) => void;
  onDecide: (kind: "approve" | "approve_constrained" | "deny") => void;
}

export function EventDetailPanel({
  event,
  replayResult,
  onReplay,
  onDecide,
}: EventDetailPanelProps): React.JSX.Element {
  if (!event) {
    return (
      <div>
        <h2 className="pane-title">Detail</h2>
        <div className="empty">Select an event.</div>
      </div>
    );
  }

  const payload = event.payload as Record<string, unknown>;
  const isApprovalRequired = event.kind === "approval_required";
  const hasConstrained =
    isApprovalRequired && payload.constrained_input !== undefined;
  const replayable =
    event.kind === "model_call" || event.kind === "context_build";

  // Tease apart the payload sections: a context_build / model_call event
  // is the natural place to surface the manifest + policy snapshot side by
  // side with the raw payload.
  const manifest = (payload.manifest ?? null) as unknown;
  const policy = (payload.policy ?? null) as unknown;

  return (
    <div>
      <h2 className="pane-title">
        <span>Detail</span>
        <span className="actions" style={{ fontFamily: "monospace" }}>
          {event.kind}
        </span>
      </h2>

      <div className="detail-body">
        <pre>{JSON.stringify(event, null, 2)}</pre>
        <div className="detail-actions">
          {isApprovalRequired && (
            <>
              <button onClick={() => onDecide("approve")}>Approve</button>
              {hasConstrained && (
                <button className="secondary" onClick={() => onDecide("approve_constrained")}>
                  Approve constrained
                </button>
              )}
              <button className="danger" onClick={() => onDecide("deny")}>
                Deny
              </button>
            </>
          )}
        </div>
      </div>

      {manifest !== null && (
        <>
          <div className="section-h">Context manifest</div>
          <div className="detail-body">
            <pre>{JSON.stringify(manifest, null, 2)}</pre>
          </div>
        </>
      )}

      {policy !== null && (
        <>
          <div className="section-h">Policy snapshot</div>
          <div className="detail-body">
            <pre>{JSON.stringify(policy, null, 2)}</pre>
          </div>
        </>
      )}

      {replayable && (
        <ReplayPanel anchor={event} result={replayResult} onResult={onReplay} />
      )}
    </div>
  );
}
