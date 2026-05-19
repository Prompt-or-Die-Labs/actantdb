import * as React from "react";
import { useCallback, useEffect, useState } from "react";
import type { ActantEvent } from "@actantdb/types";

import { api, type ReplayResponse, type StudioInfo } from "./lib/api.js";
import { ApprovalsPanel } from "./panels/ApprovalsPanel.js";
import { EventDetailPanel } from "./panels/EventDetailPanel.js";
import { RunsPanel } from "./panels/RunsPanel.js";
import { TimelinePanel } from "./panels/TimelinePanel.js";

const POLL_INTERVAL_MS = 2000;

export function App(): React.JSX.Element {
  const [info, setInfo] = useState<StudioInfo | null>(null);
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const [events, setEvents] = useState<ActantEvent[]>([]);
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
  const [replayResult, setReplayResult] = useState<ReplayResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Refresh /api/info + the currently-active run's events.
  // Polling because server.ts has no WebSocket upgrade handler today.
  // See ../GAPS.md row #6 for the follow-up.
  const refresh = useCallback(async () => {
    try {
      const next = await api.info();
      setInfo(next);
      setError(null);
      // Auto-select latest run if none selected.
      setActiveRunId((prev) => {
        if (prev) return prev;
        if (next.runs.length === 0) return null;
        return next.runs[next.runs.length - 1]!.runId;
      });
    } catch (err) {
      setError((err as Error).message);
    }
  }, []);

  // Initial + interval refresh of meta.
  useEffect(() => {
    refresh().catch(() => undefined);
    const id = window.setInterval(() => {
      refresh().catch(() => undefined);
    }, POLL_INTERVAL_MS);
    return () => window.clearInterval(id);
  }, [refresh]);

  // Pull events whenever the active run changes (and on poll tick).
  useEffect(() => {
    if (!activeRunId) {
      setEvents([]);
      return;
    }
    let cancelled = false;
    const fetchEvents = async () => {
      try {
        const r = await api.events(activeRunId);
        if (!cancelled) setEvents(r.events ?? []);
      } catch (err) {
        if (!cancelled) setError((err as Error).message);
      }
    };
    fetchEvents();
    const id = window.setInterval(fetchEvents, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [activeRunId]);

  const selectedEvent =
    selectedEventId !== null ? events.find((e) => e.id === selectedEventId) ?? null : null;

  // Clear replay result when the user navigates to a different event.
  const handleSelectEvent = useCallback((e: ActantEvent | null) => {
    setSelectedEventId(e ? e.id : null);
    setReplayResult(null);
  }, []);

  const handleSwitchRun = useCallback((runId: string) => {
    setActiveRunId(runId);
    setSelectedEventId(null);
    setReplayResult(null);
  }, []);

  // Approval decision — used by both the inline ApprovalsPanel and the
  // EventDetailPanel buttons.
  const handleDecide = useCallback(
    async (
      toolCallId: string,
      kind: "approve" | "approve_constrained" | "deny",
      hintEvent?: ActantEvent,
    ) => {
      let decision;
      if (kind === "approve") {
        decision = { decision: "approve" as const, approver: "studio", scope: "once" };
      } else if (kind === "approve_constrained") {
        const payload = (hintEvent?.payload ?? {}) as Record<string, unknown>;
        const accepted_input =
          (payload.constrained_input as unknown) ?? (payload.args as unknown);
        decision = {
          decision: "approve_constrained" as const,
          approver: "studio",
          scope: "once",
          accepted_input,
        };
      } else {
        decision = {
          decision: "deny" as const,
          approver: "studio",
          reason: "denied from Studio",
        };
      }
      try {
        await api.decide(toolCallId, decision);
        await refresh();
      } catch (err) {
        setError((err as Error).message);
      }
    },
    [refresh],
  );

  const handleReplay = useCallback(async (result: ReplayResponse) => {
    setReplayResult(result);
  }, []);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">Actant Studio</div>
        <div className="meta">
          {info ? `${info.project} · ${info.dbPath}` : "loading…"}
        </div>
      </header>
      {error && <div className="errbar">Error: {error}</div>}
      <div className="layout">
        <aside className="pane">
          <RunsPanel
            runs={info?.runs ?? []}
            activeRunId={activeRunId}
            onSelect={handleSwitchRun}
          />
        </aside>
        <section className="pane">
          <ApprovalsPanel onDecide={(id, kind) => handleDecide(id, kind)} />
          <TimelinePanel
            events={events}
            selectedEventId={selectedEventId}
            onSelect={handleSelectEvent}
          />
        </section>
        <section className="pane">
          <EventDetailPanel
            event={selectedEvent}
            replayResult={replayResult}
            onReplay={handleReplay}
            onDecide={(kind) =>
              selectedEvent &&
              handleDecide(
                String(
                  (selectedEvent.payload as Record<string, unknown>).tool_call_id ?? "",
                ),
                kind,
                selectedEvent,
              )
            }
          />
        </section>
      </div>
    </div>
  );
}
