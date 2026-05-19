import * as React from "react";
import { useState } from "react";
import type { ActantEvent, ReplayDiff } from "@actantdb/types";

import { api, type ReplayMode, type ReplayResponse } from "../lib/api.js";

interface ReplayPanelProps {
  anchor: ActantEvent;
  result: ReplayResponse | null;
  onResult: (r: ReplayResponse) => void;
}

const MODES: { value: ReplayMode; label: string }[] = [
  { value: "recorded", label: "recorded (re-emit recorded outputs)" },
  { value: "model", label: "model (re-evaluate model_call)" },
  { value: "policy", label: "policy (re-evaluate Guard verdicts)" },
  { value: "memory", label: "memory (rebuild manifest without excluded memory)" },
];

export function ReplayPanel({ anchor, result, onResult }: ReplayPanelProps): React.JSX.Element {
  const [mode, setMode] = useState<ReplayMode>("model");
  const [strict, setStrict] = useState(true);
  const [excludeMem42, setExcludeMem42] = useState(true);
  const [altMemory, setAltMemory] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const run = async () => {
    setBusy(true);
    setErr(null);
    try {
      const without_memory: string[] = [];
      if (excludeMem42) without_memory.push("mem_42_dist");
      if (altMemory.trim()) without_memory.push(altMemory.trim());
      const r = await api.replay({
        eventId: anchor.id,
        overrides: { without_memory },
        useStrictPolicy: strict,
        mode,
      });
      onResult(r);
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <div className="section-h">Replay from this event</div>
      <div className="replay-form">
        <fieldset>
          <legend>Mode</legend>
          {MODES.map((m) => (
            <label key={m.value}>
              <input
                type="radio"
                name="mode"
                value={m.value}
                checked={mode === m.value}
                onChange={() => setMode(m.value)}
              />
              {m.label}
            </label>
          ))}
        </fieldset>
        <label>
          <input
            type="checkbox"
            checked={strict}
            onChange={(e) => setStrict(e.target.checked)}
          />
          Use stricter policy (no shell.run without dist guard)
        </label>
        <label>
          <input
            type="checkbox"
            checked={excludeMem42}
            onChange={(e) => setExcludeMem42(e.target.checked)}
          />
          Exclude memory item mem_42 (the "/build and /dist" memory)
        </label>
        <label>
          Alternate memory id (optional):{" "}
          <input
            type="text"
            value={altMemory}
            placeholder="mem_…"
            onChange={(e) => setAltMemory(e.target.value)}
          />
        </label>
        <div className="detail-actions">
          <button disabled={busy} onClick={run}>
            {busy ? "Running…" : "Run replay"}
          </button>
        </div>
        {err && <div className="empty" style={{ color: "var(--error)" }}>{err}</div>}
      </div>
      {result?.diff && <DiffView diff={result.diff} />}
    </div>
  );
}

function DiffView({ diff }: { diff: ReplayDiff }): React.JSX.Element {
  return (
    <>
      <table className="diff-table">
        <thead>
          <tr>
            <th>event</th>
            <th>diff</th>
            <th>original</th>
            <th>replay</th>
          </tr>
        </thead>
        <tbody>
          {diff.entries.map((entry, i) => (
            <tr key={i}>
              <td>{entry.kind}</td>
              <td className={entry.diff}>{entry.diff}</td>
              <td>{JSON.stringify(entry.a ?? "")}</td>
              <td>{JSON.stringify(entry.b ?? "")}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="callout">
        Without the excluded memory, the planner proposed a safer command. Memory
        caused the risky proposal; Guard caught it; replay proves the link.
      </div>
    </>
  );
}
