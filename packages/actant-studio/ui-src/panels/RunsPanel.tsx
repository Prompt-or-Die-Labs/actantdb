import * as React from "react";

import type { RunSummary } from "../lib/api.js";

interface RunsPanelProps {
  runs: RunSummary[];
  activeRunId: string | null;
  onSelect: (runId: string) => void;
}

export function RunsPanel({ runs, activeRunId, onSelect }: RunsPanelProps): React.JSX.Element {
  return (
    <div>
      <h2 className="pane-title">Runs</h2>
      {runs.length === 0 ? (
        <div className="empty">No runs yet.</div>
      ) : (
        <ul className="runs-list">
          {runs.map((r) => (
            <li
              key={r.runId}
              className={r.runId === activeRunId ? "active" : ""}
              onClick={() => onSelect(r.runId)}
            >
              <div className="runid">{r.runId.slice(0, 10)}…</div>
              <small>
                {r.events} events · {r.startedAt.slice(0, 19).replace("T", " ")}
              </small>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
