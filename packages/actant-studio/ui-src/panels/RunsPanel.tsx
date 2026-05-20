import * as React from "react";

import type { RunSummary } from "../lib/api.js";

interface RunsPanelProps {
  runs: RunSummary[];
  activeRunId: string | null;
  onSelect: (runId: string) => void;
}

export function RunsPanel({ runs, activeRunId, onSelect }: RunsPanelProps): React.JSX.Element {
  const quickstart = `import { withActant } from "@actantdb/mastra";
import { demoPolicy } from "@actantdb/policy";

const agent = {
  name: "studio-quickstart",
  tools: {
    "demo.echo": {
      id: "demo.echo",
      execute: async (args) => ({ echoed: args }),
    },
  },
};

const wrapped = withActant(agent, {
  project: "studio-quickstart",
  policy: demoPolicy,
  autoApprove: true,
});

const ctx = wrapped.startRun();
ctx.recordUserMessage("Capture my first tool call");
await agent.tools["demo.echo"].execute({ text: "hello actantdb" });
ctx.finish({ ok: true });
wrapped.actant.close();`;

  return (
    <div>
      <h2 className="pane-title">Runs</h2>
      {runs.length === 0 ? (
        <div className="runs-empty-state">
          <div>
            <h3>Capture your first run</h3>
            <p>
              Run this once, then refresh Studio to inspect the tool call,
              Guard verdict, and hash-chained event rows.
            </p>
          </div>
          <pre aria-label="quickstart capture snippet">
            <code>{quickstart}</code>
          </pre>
        </div>
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
