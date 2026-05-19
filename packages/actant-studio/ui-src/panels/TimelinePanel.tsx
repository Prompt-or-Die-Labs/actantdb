import * as React from "react";
import type { ActantEvent } from "@actantdb/types";

interface TimelinePanelProps {
  events: ActantEvent[];
  selectedEventId: string | null;
  onSelect: (e: ActantEvent | null) => void;
}

interface Badge {
  cls: string;
  text: string;
}

function summarize(e: ActantEvent): string {
  const p = e.payload as Record<string, unknown>;
  switch (e.kind) {
    case "user_message_received":
      return JSON.stringify(p.text);
    case "model_call":
      return `${String(p.role ?? "")}: ${String(p.summary ?? "")}`;
    case "context_build": {
      const inc = Array.isArray(p.included) ? p.included.length : 0;
      const blk = Array.isArray(p.blocked) ? p.blocked.length : 0;
      return `${inc} included, ${blk} blocked`;
    }
    case "tool_call_requested":
      return `${String(p.tool ?? "")} ${JSON.stringify(p.args)}`;
    case "tool_call_started":
      return `${JSON.stringify(p.final_args)}`;
    case "tool_call_completed":
      return `status=${String(p.status ?? "")} ${String(p.duration_ms ?? "")}ms`;
    case "guard_verdict":
      return `${String(p.decision ?? "")} — ${String(p.reason ?? "")}`;
    case "approval_required":
      return `${String(p.tool ?? "")}${p.hint ? " hint: " + String(p.hint) : ""}`;
    case "approval_decision":
      return `${String(p.decision ?? "")}${p.approver ? " by " + String(p.approver) : ""}`;
    case "effect_observed":
    case "agent_run_finished":
      return JSON.stringify(p);
    case "agent_run_started":
      return "run started";
    default:
      return "";
  }
}

function badgeFor(e: ActantEvent): Badge {
  const p = e.payload as Record<string, unknown>;
  if (e.kind === "guard_verdict") {
    return { cls: "guard", text: String(p.decision ?? "guard") };
  }
  if (e.kind === "approval_required") return { cls: "approval", text: "approval" };
  if (e.kind === "approval_decision") {
    return { cls: "approval", text: String(p.decision ?? "decision") };
  }
  if (e.kind === "tool_call_completed") {
    const s = String(p.status ?? "");
    return { cls: s === "ok" ? "completed" : "blocked", text: s };
  }
  return { cls: "", text: "" };
}

export function TimelinePanel({
  events,
  selectedEventId,
  onSelect,
}: TimelinePanelProps): React.JSX.Element {
  return (
    <div>
      <h2 className="pane-title">Timeline</h2>
      {events.length === 0 ? (
        <div className="empty">No events for this run.</div>
      ) : (
        <div role="list" aria-label="event timeline">
          {events.map((e) => {
            const badge = badgeFor(e);
            const ts = e.created_at.slice(11, 19);
            const selected = e.id === selectedEventId;
            return (
              <div
                key={e.id}
                role="listitem"
                aria-selected={selected}
                className={"event" + (selected ? " selected" : "")}
                onClick={() => onSelect(e)}
              >
                <div className="ts">{ts}</div>
                <div className="kind">{e.kind}</div>
                <div className="summary">{summarize(e)}</div>
                <div className={"badge " + badge.cls}>{badge.text}</div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
