import * as React from "react";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { ActantEvent, ApprovalDecision } from "@actantdb/types";

import { api, type ApprovalRecord, type ReplayResponse, type StudioInfo } from "./lib/api.js";
import { ApprovalsPanel } from "./panels/ApprovalsPanel.js";
import { EventDetailPanel } from "./panels/EventDetailPanel.js";
import { RunsPanel } from "./panels/RunsPanel.js";
import { TimelinePanel } from "./panels/TimelinePanel.js";

const POLL_INTERVAL_MS = 2000;

type TabId =
  | "overview"
  | "tables"
  | "sql"
  | "database"
  | "api"
  | "auth"
  | "storage"
  | "realtime"
  | "functions"
  | "logs"
  | "reports"
  | "backups"
  | "branches"
  | "settings";
type DbTable = "runs" | "events" | "approvals" | "actors";
type JsonValue = string | number | boolean | null | JsonValue[] | { [key: string]: JsonValue };
type PayloadRecord = { [key: string]: JsonValue };
type TableRow = { id: string; cells: string[] };
type FeatureStatus = "Available" | "Read-only" | "Configure in code";

interface FeatureCardModel {
  title: string;
  status: FeatureStatus;
  description: string;
  rows: Array<[string, string]>;
}

interface ChatMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: string;
  rawEvent?: ActantEvent;
  subEvents: ActantEvent[];
  pendingApproval?: ApprovalRecord;
}

const navGroups: Array<{ label: string; items: Array<{ id: TabId; label: string }> }> = [
  {
    label: "Project",
    items: [
      { id: "overview", label: "Project Home" },
      { id: "tables", label: "Table Editor" },
      { id: "sql", label: "SQL Editor" },
      { id: "database", label: "Database" },
    ],
  },
  {
    label: "Build",
    items: [
      { id: "api", label: "API Docs" },
      { id: "auth", label: "Auth" },
      { id: "storage", label: "Storage" },
      { id: "realtime", label: "Realtime" },
      { id: "functions", label: "Edge Functions" },
    ],
  },
  {
    label: "Monitor",
    items: [
      { id: "logs", label: "Logs" },
      { id: "reports", label: "Reports" },
      { id: "backups", label: "Backups" },
    ],
  },
  {
    label: "Configure",
    items: [
      { id: "branches", label: "Branches" },
      { id: "settings", label: "Settings" },
    ],
  },
];

const tableHeaders: Record<DbTable, string[]> = {
  runs: ["Run ID", "Events", "Started"],
  events: ["Event ID", "Run", "Kind", "Time", "Payload"],
  approvals: ["Tool Call", "Run", "Status", "Tool", "Created"],
  actors: ["Actor / Role", "Runs", "Events"],
};

export function App(): React.JSX.Element {
  const [info, setInfo] = useState<StudioInfo | null>(null);
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const [events, setEvents] = useState<ActantEvent[]>([]);
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
  const [replayResult, setReplayResult] = useState<ReplayResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>("overview");
  const [selectedDbTable, setSelectedDbTable] = useState<DbTable>("runs");
  const [allEvents, setAllEvents] = useState<ActantEvent[]>([]);
  const [approvals, setApprovals] = useState<ApprovalRecord[]>([]);
  const [dbSearchQuery, setDbSearchQuery] = useState("");
  const [expandedMessages, setExpandedMessages] = useState<Record<string, boolean>>({});

  const runs = info?.runs ?? [];
  const activeRun = runs.find((run) => run.runId === activeRunId) ?? null;
  const selectedEvent =
    selectedEventId !== null ? events.find((event) => event.id === selectedEventId) ?? null : null;

  const refresh = useCallback(async () => {
    try {
      const nextInfo = await api.info();
      const nextApprovals = await api.approvals();
      setInfo(nextInfo);
      setApprovals(nextApprovals.approvals ?? []);
      setError(null);
      setActiveRunId((previous) => {
        if (previous && nextInfo.runs.some((run) => run.runId === previous)) return previous;
        return nextInfo.runs.at(-1)?.runId ?? null;
      });
    } catch (err) {
      setError(errorMessage(err));
    }
  }, []);

  const fetchAllEvents = useCallback(async () => {
    try {
      const response = await api.events();
      setAllEvents(response.events ?? []);
    } catch (err) {
      setError(errorMessage(err));
    }
  }, []);

  useEffect(() => {
    refresh().catch((err) => setError(errorMessage(err)));
    fetchAllEvents().catch((err) => setError(errorMessage(err)));
    const id = window.setInterval(() => {
      refresh().catch((err) => setError(errorMessage(err)));
      fetchAllEvents().catch((err) => setError(errorMessage(err)));
    }, POLL_INTERVAL_MS);
    return () => window.clearInterval(id);
  }, [fetchAllEvents, refresh]);

  useEffect(() => {
    if (!activeRunId) {
      setEvents([]);
      return;
    }

    let cancelled = false;
    const fetchEvents = async () => {
      try {
        const response = await api.events(activeRunId);
        if (!cancelled) {
          setEvents(response.events ?? []);
          setError(null);
        }
      } catch (err) {
        if (!cancelled) setError(errorMessage(err));
      }
    };

    fetchEvents().catch((err) => setError(errorMessage(err)));
    const id = window.setInterval(fetchEvents, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [activeRunId]);

  const pendingApprovals = useMemo(
    () => approvals.filter((approval) => approval.status === "pending"),
    [approvals],
  );

  const eventKindCounts = useMemo(() => countByKind(allEvents), [allEvents]);
  const eventKindRows = useMemo(
    () => Object.entries(eventKindCounts).sort((a, b) => b[1] - a[1]).slice(0, 8),
    [eventKindCounts],
  );

  const actorCount = useMemo(() => {
    const actors = new Set<string>();
    for (const event of allEvents) {
      const payload = payloadRecord(event);
      actors.add(valueText(payload.actor ?? payload.role ?? roleFallback(event.kind)));
    }
    return actors.size;
  }, [allEvents]);

  const chainHealth = useMemo(() => {
    if (allEvents.length === 0) return "Ready";
    const valid = allEvents.filter((event) => event.chain_hash.length === 64).length;
    return `${valid}/${allEvents.length}`;
  }, [allEvents]);

  const toolErrors = useMemo(
    () =>
      allEvents.filter((event) => {
        if (event.kind !== "tool_call_completed") return false;
        return valueText(payloadRecord(event).status) === "error";
      }).length,
    [allEvents],
  );

  const guardVerdicts = useMemo(
    () => allEvents.filter((event) => event.kind === "guard_verdict").length,
    [allEvents],
  );

  const tableRows = useMemo(
    () => getDbTableRows(selectedDbTable, dbSearchQuery, runs, allEvents, approvals),
    [allEvents, approvals, dbSearchQuery, runs, selectedDbTable],
  );

  const chatMessages = useMemo(
    () => buildChatMessages(events, approvals),
    [approvals, events],
  );

  const handleSelectEvent = useCallback((event: ActantEvent | null) => {
    setSelectedEventId(event ? event.id : null);
    setReplayResult(null);
  }, []);

  const handleSwitchRun = useCallback((runId: string) => {
    setActiveRunId(runId);
    setSelectedEventId(null);
    setReplayResult(null);
  }, []);

  const handleDecide = useCallback(
    async (
      toolCallId: string,
      kind: "approve" | "approve_constrained" | "deny",
      hintEvent?: ActantEvent,
    ) => {
      const decision = buildDecision(kind, hintEvent);
      try {
        await api.decide(toolCallId, decision);
        await refresh();
        await fetchAllEvents();
      } catch (err) {
        setError(errorMessage(err));
      }
    },
    [fetchAllEvents, refresh],
  );

  const handleReplay = useCallback((result: ReplayResponse) => {
    setReplayResult(result);
  }, []);

  const toggleMessage = useCallback((id: string) => {
    setExpandedMessages((previous) => ({ ...previous, [id]: !previous[id] }));
  }, []);

  const openSelectedEvent = useCallback((event: ActantEvent) => {
    if (event.run_id !== activeRunId) setActiveRunId(event.run_id);
    setSelectedEventId(event.id);
    setReplayResult(null);
    setActiveTab("realtime");
  }, [activeRunId]);

  return (
    <div className="studio-shell">
      <aside className="studio-sidebar">
        <div className="studio-brand">
          <div className="studio-mark">a</div>
          <div>
            <strong>actantdb</strong>
            <span>Studio</span>
          </div>
        </div>

        <nav className="studio-nav" aria-label="Studio views">
          {navGroups.map((group) => (
            <div className="studio-nav-group" key={group.label}>
              <div className="studio-nav-heading">{group.label}</div>
              {group.items.map((item) => (
                <button
                  key={item.id}
                  className={activeTab === item.id ? "active" : ""}
                  type="button"
                  onClick={() => setActiveTab(item.id)}
                >
                  {item.label}
                </button>
              ))}
            </div>
          ))}
        </nav>

        <div className="studio-sidebar-status">
          <div>
            <span>Project</span>
            <strong title={info?.project}>{info?.project ?? "loading"}</strong>
          </div>
          <div>
            <span>SQLite</span>
            <strong title={info?.dbPath}>{shortPath(info?.dbPath ?? "loading")}</strong>
          </div>
          <div style={{ display: "none" }} data-testid="test-meta">
            {info?.project} · {info?.dbPath}
          </div>
        </div>
      </aside>

      <main className="studio-main">
        <header className="studio-topbar">
          <div>
            <p>
              actantdb / {info?.project ?? "loading"} / main
              {activeRun && <span className="studio-topbar-meta">Run {activeRun.runId.slice(0, 12)}...</span>}
            </p>
            <h1>{tabTitle(activeTab)}</h1>
          </div>
          <div className="studio-topbar-actions">
            <span className="branch-pill">main</span>
            {error && <div className="studio-error">Error: {error}</div>}
            <button className="secondary" type="button" onClick={refresh}>
              Refresh
            </button>
          </div>
        </header>

        <section className="studio-workspace">
          {activeTab === "overview" && (
            <OverviewView
              actorCount={actorCount}
              chainHealth={chainHealth}
              events={events}
              eventKindRows={eventKindRows}
              eventTotal={allEvents.length}
              guardVerdicts={guardVerdicts}
              onDecide={handleDecide}
              onReplay={handleReplay}
              onSelectEvent={handleSelectEvent}
              onSwitchRun={handleSwitchRun}
              pendingApprovals={pendingApprovals}
              replayResult={replayResult}
              runs={runs}
              selectedEvent={selectedEvent}
              selectedEventId={selectedEventId}
              toolErrors={toolErrors}
              activeRunId={activeRunId}
              onNavigate={setActiveTab}
            />
          )}

          {activeTab === "tables" && (
            <TablesView
              rows={tableRows}
              searchQuery={dbSearchQuery}
              selectedTable={selectedDbTable}
              setSearchQuery={setDbSearchQuery}
              setSelectedTable={setSelectedDbTable}
            />
          )}

          {activeTab === "sql" && <SqlEditorView project={info?.project ?? "my-project"} />}

          {activeTab === "database" && (
            <DatabaseView
              actorCount={actorCount}
              chainHealth={chainHealth}
              eventKindRows={eventKindRows}
              eventTotal={allEvents.length}
              runCount={runs.length}
            />
          )}

          {activeTab === "api" && <ApiDocsView project={info?.project ?? "my-project"} />}

          {activeTab === "auth" && (
            <AuthView
              guardVerdicts={guardVerdicts}
              onDecide={handleDecide}
              pendingApprovals={pendingApprovals}
              toolErrors={toolErrors}
            />
          )}

          {activeTab === "storage" && (
            <StorageView
              chainHealth={chainHealth}
              dbPath={info?.dbPath ?? ""}
              eventTotal={allEvents.length}
              runCount={runs.length}
            />
          )}

          {activeTab === "realtime" && (
            <RealtimeView
              events={events}
              onSelectEvent={handleSelectEvent}
              selectedEventId={selectedEventId}
            />
          )}

          {activeTab === "functions" && (
            <FunctionsView
              chatMessages={chatMessages}
              expandedMessages={expandedMessages}
              onDecide={handleDecide}
              onSwitchRun={handleSwitchRun}
              onToggleMessage={toggleMessage}
              runs={runs}
              activeRunId={activeRunId}
            />
          )}

          {activeTab === "logs" && (
            <LogsView
              events={allEvents}
              onOpenEvent={openSelectedEvent}
            />
          )}

          {activeTab === "reports" && (
            <TelemetryView
              actorCount={actorCount}
              chainHealth={chainHealth}
              eventKindRows={eventKindRows}
              eventTotal={allEvents.length}
              guardVerdicts={guardVerdicts}
              pendingApprovalCount={pendingApprovals.length}
              runCount={runs.length}
              toolErrors={toolErrors}
              project={info?.project ?? "my-project"}
            />
          )}

          {activeTab === "backups" && (
            <BackupsView dbPath={info?.dbPath ?? ""} eventTotal={allEvents.length} runCount={runs.length} />
          )}

          {activeTab === "branches" && <BranchesView project={info?.project ?? "my-project"} />}

          {activeTab === "settings" && (
            <SettingsView
              dbPath={info?.dbPath ?? ""}
              project={info?.project ?? "my-project"}
              pollIntervalMs={POLL_INTERVAL_MS}
            />
          )}
        </section>
      </main>
    </div>
  );
}

function OverviewView(props: {
  activeRunId: string | null;
  actorCount: number;
  chainHealth: string;
  events: ActantEvent[];
  eventKindRows: Array<[string, number]>;
  eventTotal: number;
  guardVerdicts: number;
  onDecide: (toolCallId: string, kind: "approve" | "approve_constrained" | "deny", hintEvent?: ActantEvent) => Promise<void>;
  onReplay: (result: ReplayResponse) => void;
  onNavigate: (tab: TabId) => void;
  onSelectEvent: (event: ActantEvent | null) => void;
  onSwitchRun: (runId: string) => void;
  pendingApprovals: ApprovalRecord[];
  replayResult: ReplayResponse | null;
  runs: StudioInfo["runs"];
  selectedEvent: ActantEvent | null;
  selectedEventId: string | null;
  toolErrors: number;
}): React.JSX.Element {
  return (
    <div className="overview-grid">
      <section className="metric-strip" aria-label="Ledger health">
        <Metric label="Runs" value={String(props.runs.length)} tone="accent" />
        <Metric label="Events" value={String(props.eventTotal)} tone="neutral" />
        <Metric label="Pending approvals" value={String(props.pendingApprovals.length)} tone="warn" />
        <Metric label="Chain health" value={props.chainHealth} tone="ok" />
      </section>

      <section className="dashboard-panel project-launcher">
        <PanelTitle eyebrow="Project" title="Supabase-style workspace" meta="14 sections" />
        <div className="feature-launcher-grid">
          {navGroups.flatMap((group) => group.items).map((item) => (
            <button
              key={item.id}
              className={item.id === "overview" ? "active" : "secondary"}
              type="button"
              onClick={() => props.onNavigate(item.id)}
            >
              <span>{item.label}</span>
              <em>{featureHint(item.id)}</em>
            </button>
          ))}
        </div>
      </section>

      <section className="dashboard-band command-band">
        <div className="dashboard-panel run-column">
          <RunsPanel
            runs={props.runs}
            activeRunId={props.activeRunId}
            onSelect={props.onSwitchRun}
          />
        </div>

        <div className="dashboard-panel event-column">
          <PanelTitle eyebrow="Live run" title="Event stream" meta={`${props.events.length} rows`} />
          <TimelinePanel
            events={props.events}
            selectedEventId={props.selectedEventId}
            onSelect={props.onSelectEvent}
          />
        </div>

        <div className="dashboard-panel detail-column">
          <PanelTitle eyebrow="Replay" title="Decision detail" />
          <EventDetailPanel
            event={props.selectedEvent}
            replayResult={props.replayResult}
            onReplay={props.onReplay}
            onDecide={(kind) => {
              if (!props.selectedEvent) return;
              void props.onDecide(
                valueText(payloadRecord(props.selectedEvent).tool_call_id),
                kind,
                props.selectedEvent,
              );
            }}
          />
        </div>
      </section>

      <section className="dashboard-band lower-band">
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Attention" title="Approval queue" meta={`${props.pendingApprovals.length} pending`} />
          <ApprovalQueue approvals={props.pendingApprovals} onDecide={props.onDecide} />
        </div>
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Distribution" title="Event mix" meta={`${props.guardVerdicts} guard verdicts`} />
          <EventBars rows={props.eventKindRows} />
        </div>
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Runtime" title="Signal summary" meta={`${props.actorCount} actors`} />
          <SignalSummary toolErrors={props.toolErrors} guardVerdicts={props.guardVerdicts} />
        </div>
      </section>
    </div>
  );
}

function TimelineView(props: {
  activeRunId: string | null;
  events: ActantEvent[];
  onDecide: (toolCallId: string, kind: "approve" | "approve_constrained" | "deny", hintEvent?: ActantEvent) => Promise<void>;
  onReplay: (result: ReplayResponse) => void;
  onSelectEvent: (event: ActantEvent | null) => void;
  onSwitchRun: (runId: string) => void;
  replayResult: ReplayResponse | null;
  runs: StudioInfo["runs"];
  selectedEvent: ActantEvent | null;
  selectedEventId: string | null;
}): React.JSX.Element {
  return (
    <div className="timeline-workbench">
      <div className="dashboard-panel">
        <RunsPanel runs={props.runs} activeRunId={props.activeRunId} onSelect={props.onSwitchRun} />
      </div>
      <div className="dashboard-panel">
        <ApprovalsPanel onDecide={(id, kind) => props.onDecide(id, kind)} />
        <TimelinePanel
          events={props.events}
          selectedEventId={props.selectedEventId}
          onSelect={props.onSelectEvent}
        />
      </div>
      <div className="dashboard-panel">
        <EventDetailPanel
          event={props.selectedEvent}
          replayResult={props.replayResult}
          onReplay={props.onReplay}
          onDecide={(kind) => {
            if (!props.selectedEvent) return;
            void props.onDecide(
              valueText(payloadRecord(props.selectedEvent).tool_call_id),
              kind,
              props.selectedEvent,
            );
          }}
        />
      </div>
    </div>
  );
}

function SessionsView(props: {
  activeRunId: string | null;
  chatMessages: ChatMessage[];
  expandedMessages: Record<string, boolean>;
  onDecide: (toolCallId: string, kind: "approve" | "approve_constrained" | "deny", hintEvent?: ActantEvent) => Promise<void>;
  onSwitchRun: (runId: string) => void;
  onToggleMessage: (id: string) => void;
  runs: StudioInfo["runs"];
}): React.JSX.Element {
  return (
    <div className="session-grid">
      <div className="dashboard-panel">
        <RunsPanel runs={props.runs} activeRunId={props.activeRunId} onSelect={props.onSwitchRun} />
      </div>
      <div className="dashboard-panel session-thread-panel">
        <PanelTitle eyebrow="Session" title="Interaction feed" meta={`${props.chatMessages.length} messages`} />
        {props.chatMessages.length === 0 ? (
          <div className="empty-state-block">
            <strong>No interaction events</strong>
            <span>Select a run with user messages or capture a new run.</span>
          </div>
        ) : (
          <div className="session-thread">
            {props.chatMessages.map((message) => (
              <SessionMessage
                key={message.id}
                message={message}
                expanded={Boolean(props.expandedMessages[message.id])}
                onDecide={props.onDecide}
                onToggle={() => props.onToggleMessage(message.id)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function TablesView(props: {
  rows: TableRow[];
  searchQuery: string;
  selectedTable: DbTable;
  setSearchQuery: (value: string) => void;
  setSelectedTable: (table: DbTable) => void;
}): React.JSX.Element {
  return (
    <div className="tables-workbench">
      <aside className="table-rail">
        <h2>Tables</h2>
        {(["runs", "events", "approvals", "actors"] as DbTable[]).map((table) => (
          <button
            key={table}
            className={props.selectedTable === table ? "active" : ""}
            type="button"
            onClick={() => {
              props.setSelectedTable(table);
              props.setSearchQuery("");
            }}
          >
            {table}
          </button>
        ))}
      </aside>
      <div className="dashboard-panel table-panel">
        <div className="table-toolbar">
          <div>
            <span>{props.selectedTable}</span>
            <strong>{props.rows.length} rows</strong>
          </div>
          <input
            type="search"
            value={props.searchQuery}
            placeholder={`Search ${props.selectedTable}`}
            onChange={(event) => props.setSearchQuery(event.target.value)}
          />
        </div>
        <div className="db-table-scroller">
          <table className="db-data-table">
            <thead>
              <tr>
                {tableHeaders[props.selectedTable].map((header) => (
                  <th key={header}>{header}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {props.rows.length === 0 ? (
                <tr>
                  <td colSpan={tableHeaders[props.selectedTable].length} className="table-empty">
                    No records matching query.
                  </td>
                </tr>
              ) : (
                props.rows.map((row) => (
                  <tr key={row.id}>
                    {row.cells.map((cell) => (
                      <td key={`${row.id}-${cell}`}>{cell}</td>
                    ))}
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function SqlEditorView(props: { project: string }): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Saved queries",
            status: "Read-only",
            description: "ActantDB ships useful ledger queries as copyable starters until Studio gets a write-safe SQL runner.",
            rows: [
              ["recent_events", "Latest append-only event rows"],
              ["pending_approvals", "Guard decisions waiting on an operator"],
              ["run_summary", "Run counts grouped by project"],
            ],
          },
          {
            title: "SQL safety",
            status: "Configure in code",
            description: "The dashboard exposes query patterns without executing arbitrary SQL against the local ledger.",
            rows: [
              ["mode", "Read-only examples"],
              ["store", props.project],
              ["driver", "SQLite / Postgres compatible schema"],
            ],
          },
        ]}
        title="SQL Editor"
      />
      <div className="sql-editor-layout">
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Editor" title="Ledger query" meta="copyable" />
          <pre className="sql-editor">{`select
  id,
  run_id,
  kind,
  sensitivity,
  created_at
from events
order by created_at desc
limit 100;`}</pre>
        </div>
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Saved" title="Query library" />
          <KeyValueList
            rows={[
              ["Event timeline", "events ordered by created_at"],
              ["Approval queue", "approval_required without approval_decision"],
              ["Chain audit", "chain_hash and prev_chain_hash continuity"],
              ["Tool latency", "tool_call_completed duration_ms"],
            ]}
          />
        </div>
      </div>
    </div>
  );
}

function DatabaseView(props: {
  actorCount: number;
  chainHealth: string;
  eventKindRows: Array<[string, number]>;
  eventTotal: number;
  runCount: number;
}): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Schemas",
            status: "Available",
            description: "Runs, events, approvals, and actors are exposed as browsable Studio tables.",
            rows: [
              ["runs", String(props.runCount)],
              ["events", String(props.eventTotal)],
              ["actors", String(props.actorCount)],
            ],
          },
          {
            title: "Integrity",
            status: "Available",
            description: "The hash chain is the database health signal for an append-only ActantDB ledger.",
            rows: [
              ["chain health", props.chainHealth],
              ["event table", "append-only"],
              ["IDs", "TEXT"],
            ],
          },
          {
            title: "Extensions",
            status: "Configure in code",
            description: "Public types are generated from actant-contracts; database shape follows the contract crate.",
            rows: [
              ["types", "actant-contracts"],
              ["bindings", "@actantdb/types"],
              ["mode", "embedded first"],
            ],
          },
        ]}
        title="Database"
      />
      <div className="dashboard-panel">
        <PanelTitle eyebrow="Distribution" title="Event frequency" meta={props.chainHealth} />
        <EventBars rows={props.eventKindRows} />
      </div>
    </div>
  );
}

function ApiDocsView(props: { project: string }): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Client libraries",
            status: "Available",
            description: "Use the wrapper packages instead of hand-writing event rows.",
            rows: [
              ["Mastra", "@actantdb/mastra"],
              ["OpenAI", "@actantdb/openai"],
              ["AI SDK", "@actantdb/ai-sdk"],
            ],
          },
          {
            title: "Studio API",
            status: "Read-only",
            description: "The local Studio server exposes project info, event streams, approvals, and replay.",
            rows: [
              ["GET", "/api/info"],
              ["GET", "/api/events?run=<id>"],
              ["POST", "/api/replay"],
            ],
          },
        ]}
        title="API Docs"
      />
      <div className="dashboard-panel">
        <PanelTitle eyebrow="Quickstart" title="Instrument an agent" meta={props.project} />
        <pre className="code-snippet-box">{`import { withActant } from "@actantdb/mastra";

const agent = withActant(mastraAgent, {
  project: "${props.project}",
  autoApprove: false,
});

await agent.run({ message: "Ship safely" });`}</pre>
      </div>
    </div>
  );
}

function AuthView(props: {
  guardVerdicts: number;
  onDecide: (toolCallId: string, kind: "approve" | "approve_constrained" | "deny") => Promise<void>;
  pendingApprovals: ApprovalRecord[];
  toolErrors: number;
}): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Policies",
            status: "Available",
            description: "Guard verdicts and approvals are ActantDB's equivalent of a project auth and policy surface.",
            rows: [
              ["guard verdicts", String(props.guardVerdicts)],
              ["pending approvals", String(props.pendingApprovals.length)],
              ["mode", "operator mediated"],
            ],
          },
          {
            title: "Users",
            status: "Configure in code",
            description: "Studio records actor and role metadata from events; external identity stays in the host app.",
            rows: [
              ["actor source", "event payload"],
              ["scope", "host application"],
              ["permissions", "policy layer"],
            ],
          },
        ]}
        title="Auth"
      />
      <section className="dashboard-band lower-band">
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Queue" title="Approvals" meta={`${props.pendingApprovals.length} pending`} />
          <ApprovalQueue approvals={props.pendingApprovals} onDecide={props.onDecide} />
        </div>
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Risk" title="Guard activity" />
          <SignalSummary toolErrors={props.toolErrors} guardVerdicts={props.guardVerdicts} />
        </div>
      </section>
    </div>
  );
}

function StorageView(props: {
  chainHealth: string;
  dbPath: string;
  eventTotal: number;
  runCount: number;
}): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Ledger store",
            status: "Available",
            description: "Embedded Studio uses the local event database as the primary artifact store.",
            rows: [
              ["runs", String(props.runCount)],
              ["events", String(props.eventTotal)],
              ["chain", props.chainHealth],
            ],
          },
          {
            title: "Objects",
            status: "Configure in code",
            description: "Large artifacts belong in the host app or sync destinations; ledger rows keep references and hashes.",
            rows: [
              ["default", "SQLite"],
              ["external", "sync destinations"],
              ["path", shortPath(props.dbPath || "not loaded")],
            ],
          },
        ]}
        title="Storage"
      />
      <div className="dashboard-panel">
        <PanelTitle eyebrow="Location" title="Local store" />
        <KeyValueList rows={[["SQLite path", props.dbPath || "loading"], ["Mode", "embedded"], ["Retention", "local project policy"]]} />
      </div>
    </div>
  );
}

function RealtimeView(props: {
  events: ActantEvent[];
  onSelectEvent: (event: ActantEvent | null) => void;
  selectedEventId: string | null;
}): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Live stream",
            status: "Available",
            description: "Studio polls the local event API and renders the active run as it grows.",
            rows: [
              ["interval", `${POLL_INTERVAL_MS}ms`],
              ["active events", String(props.events.length)],
              ["source", "/api/events?run=<id>"],
            ],
          },
          {
            title: "Channels",
            status: "Configure in code",
            description: "Subscription and broker behavior is owned by the ActantDB runtime packages.",
            rows: [
              ["event channel", "run_id"],
              ["selection", props.selectedEventId ?? "none"],
              ["transport", "HTTP poll / server stream capable"],
            ],
          },
        ]}
        title="Realtime"
      />
      <div className="dashboard-panel">
        <PanelTitle eyebrow="Stream" title="Active run events" meta={`${props.events.length} rows`} />
        <TimelinePanel events={props.events} selectedEventId={props.selectedEventId} onSelect={props.onSelectEvent} />
      </div>
    </div>
  );
}

function FunctionsView(props: {
  activeRunId: string | null;
  chatMessages: ChatMessage[];
  expandedMessages: Record<string, boolean>;
  onDecide: (toolCallId: string, kind: "approve" | "approve_constrained" | "deny", hintEvent?: ActantEvent) => Promise<void>;
  onSwitchRun: (runId: string) => void;
  onToggleMessage: (id: string) => void;
  runs: StudioInfo["runs"];
}): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Run handlers",
            status: "Available",
            description: "Agent runs, tool calls, and replay are the ActantDB equivalent of function invocation traces.",
            rows: [
              ["runs", String(props.runs.length)],
              ["messages", String(props.chatMessages.length)],
              ["active run", props.activeRunId ?? "none"],
            ],
          },
          {
            title: "Replay",
            status: "Available",
            description: "Replay can re-evaluate selected model and context events under stricter policy.",
            rows: [
              ["mode", "recorded / model / policy / memory"],
              ["trigger", "event detail"],
              ["output", "diff table"],
            ],
          },
        ]}
        title="Edge Functions"
      />
      <SessionsView
        activeRunId={props.activeRunId}
        chatMessages={props.chatMessages}
        expandedMessages={props.expandedMessages}
        onDecide={props.onDecide}
        onSwitchRun={props.onSwitchRun}
        onToggleMessage={props.onToggleMessage}
        runs={props.runs}
      />
    </div>
  );
}

function LogsView(props: {
  events: ActantEvent[];
  onOpenEvent: (event: ActantEvent) => void;
}): React.JSX.Element {
  const rows = props.events.slice(-80).reverse();
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Logs Explorer",
            status: "Available",
            description: "Every ledger event is shown as a queryable log source with run, kind, timestamp, and payload summary.",
            rows: [
              ["sources", "runs, events, approvals, actors"],
              ["rows loaded", String(rows.length)],
              ["selection", "opens in Realtime"],
            ],
          },
        ]}
        title="Logs"
      />
      <div className="dashboard-panel logs-panel">
        <PanelTitle eyebrow="Explorer" title="Recent ledger events" meta={`${rows.length} rows`} />
        {rows.length === 0 ? (
          <div className="empty-state-block">
            <strong>No logs yet</strong>
            <span>Capture a run to populate the log explorer.</span>
          </div>
        ) : (
          <div className="log-row-list">
            {rows.map((event) => (
              <button key={event.id} className="log-row" type="button" onClick={() => props.onOpenEvent(event)}>
                <span>{event.created_at.slice(11, 19)}</span>
                <strong>{event.kind}</strong>
                <em>{eventSummary(event)}</em>
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function BackupsView(props: { dbPath: string; eventTotal: number; runCount: number }): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Snapshots",
            status: "Configure in code",
            description: "ActantDB keeps a local ledger; backup cadence belongs to the project storage or sync layer.",
            rows: [
              ["runs", String(props.runCount)],
              ["events", String(props.eventTotal)],
              ["source", shortPath(props.dbPath || "not loaded")],
            ],
          },
          {
            title: "Restore posture",
            status: "Read-only",
            description: "Studio surfaces backup context but does not mutate or restore the local database from the browser.",
            rows: [
              ["restore", "CLI / storage layer"],
              ["integrity", "hash chain"],
              ["confirmation", "required outside Studio"],
            ],
          },
        ]}
        title="Backups"
      />
      <div className="dashboard-panel">
        <PanelTitle eyebrow="Runbook" title="Backup command" />
        <pre className="code-snippet-box">{`sqlite3 "${props.dbPath || "events.sqlite"}" ".backup actantdb-backup.sqlite"`}</pre>
      </div>
    </div>
  );
}

function BranchesView(props: { project: string }): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "Branch selector",
            status: "Configure in code",
            description: "The Studio top bar mirrors Supabase branch context, but ActantDB branch state follows the repo/worktree.",
            rows: [
              ["project", props.project],
              ["current branch", "main"],
              ["preview branches", "worktree-managed"],
            ],
          },
          {
            title: "Merge review",
            status: "Read-only",
            description: "Use Git and CI for branch merge review; Studio keeps ledger inspection local.",
            rows: [
              ["schema changes", "actant-contracts"],
              ["UI changes", "Studio bundle"],
              ["promotion", "git workflow"],
            ],
          },
        ]}
        title="Branches"
      />
      <div className="dashboard-panel">
        <PanelTitle eyebrow="Branches" title="Environment map" />
        <KeyValueList rows={[["Production", "main"], ["Preview", "current worktree"], ["Local database", "per project store-dir"]]} />
      </div>
    </div>
  );
}

function SettingsView(props: {
  dbPath: string;
  pollIntervalMs: number;
  project: string;
}): React.JSX.Element {
  return (
    <div className="supabase-page">
      <FeatureSurface
        cards={[
          {
            title: "General",
            status: "Available",
            description: "Project metadata and local runtime settings match the shape of a hosted project settings page.",
            rows: [
              ["project", props.project],
              ["poll interval", `${props.pollIntervalMs}ms`],
              ["database", shortPath(props.dbPath || "loading")],
            ],
          },
          {
            title: "Security",
            status: "Configure in code",
            description: "Secrets, network policy, and auth providers remain host-application concerns.",
            rows: [
              ["secrets", "host app"],
              ["network", "local server"],
              ["policy", "@actantdb/policy"],
            ],
          },
        ]}
        title="Settings"
      />
      <div className="dashboard-panel">
        <PanelTitle eyebrow="Project" title="Configuration" />
        <KeyValueList rows={[["Name", props.project], ["Database path", props.dbPath || "loading"], ["Studio mode", "local"], ["Refresh", `${props.pollIntervalMs}ms`]]} />
      </div>
    </div>
  );
}

function FeatureSurface(props: { cards: FeatureCardModel[]; title: string }): React.JSX.Element {
  return (
    <section className="dashboard-panel feature-surface">
      <PanelTitle eyebrow="Workspace" title={props.title} meta={`${props.cards.length} cards`} />
      <div className="feature-grid">
        {props.cards.map((card) => (
          <FeatureCard card={card} key={card.title} />
        ))}
      </div>
    </section>
  );
}

function FeatureCard(props: { card: FeatureCardModel }): React.JSX.Element {
  return (
    <article className="feature-card">
      <header>
        <strong>{props.card.title}</strong>
        <span className={`feature-status ${statusClass(props.card.status)}`}>{props.card.status}</span>
      </header>
      <p>{props.card.description}</p>
      <KeyValueList rows={props.card.rows} />
    </article>
  );
}

function KeyValueList(props: { rows: Array<[string, string]> }): React.JSX.Element {
  return (
    <dl className="key-value-list">
      {props.rows.map(([key, value]) => (
        <React.Fragment key={`${key}-${value}`}>
          <dt>{key}</dt>
          <dd>{value}</dd>
        </React.Fragment>
      ))}
    </dl>
  );
}

function TelemetryView(props: {
  actorCount: number;
  chainHealth: string;
  eventKindRows: Array<[string, number]>;
  eventTotal: number;
  guardVerdicts: number;
  pendingApprovalCount: number;
  project: string;
  runCount: number;
  toolErrors: number;
}): React.JSX.Element {
  return (
    <div className="telemetry-workbench">
      <section className="metric-strip">
        <Metric label="Runs" value={String(props.runCount)} tone="accent" />
        <Metric label="Events" value={String(props.eventTotal)} tone="neutral" />
        <Metric label="Approvals" value={String(props.pendingApprovalCount)} tone="warn" />
        <Metric label="Actors" value={String(props.actorCount)} tone="ok" />
      </section>
      <section className="dashboard-band lower-band">
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Ledger" title="Event frequency" meta={props.chainHealth} />
          <EventBars rows={props.eventKindRows} />
        </div>
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Risk" title="Guard activity" meta={`${props.guardVerdicts} verdicts`} />
          <SignalSummary toolErrors={props.toolErrors} guardVerdicts={props.guardVerdicts} />
        </div>
        <div className="dashboard-panel">
          <PanelTitle eyebrow="Install" title="Integration snippet" meta={props.project} />
          <pre className="code-snippet-box">{`import { withActant } from "@actantdb/mastra";

const wrapped = withActant(agent, {
  project: "${props.project}",
  autoApprove: false,
});

await wrapped.run({ message: "Ship safely" });`}</pre>
        </div>
      </section>
    </div>
  );
}

function Metric(props: { label: string; value: string; tone: "accent" | "neutral" | "warn" | "ok" }): React.JSX.Element {
  return (
    <div className={`metric-card ${props.tone}`}>
      <span>{props.label}</span>
      <strong>{props.value}</strong>
    </div>
  );
}

function PanelTitle(props: { eyebrow: string; title: string; meta?: string }): React.JSX.Element {
  return (
    <div className="panel-title-row">
      <div>
        <span>{props.eyebrow}</span>
        <h2>{props.title}</h2>
      </div>
      {props.meta && <em>{props.meta}</em>}
    </div>
  );
}

function ApprovalQueue(props: {
  approvals: ApprovalRecord[];
  onDecide: (toolCallId: string, kind: "approve" | "approve_constrained" | "deny") => Promise<void>;
}): React.JSX.Element {
  if (props.approvals.length === 0) {
    return (
      <div className="empty-state-block">
        <strong>No pending approvals</strong>
        <span>Guard decisions are clear for the current workspace.</span>
      </div>
    );
  }

  return (
    <div className="approval-list">
      {props.approvals.map((approval) => (
        <div className="approval-item" key={approval.toolCallId}>
          <div>
            <strong>{approval.request.tool}</strong>
            <span>{compactJson(approval.request.args)}</span>
          </div>
          <div className="approval-actions">
            <button type="button" onClick={() => props.onDecide(approval.toolCallId, "approve")}>
              Approve
            </button>
            {approval.request.constrained_input !== undefined && (
              <button
                className="secondary"
                type="button"
                onClick={() => props.onDecide(approval.toolCallId, "approve_constrained")}
              >
                Constrain
              </button>
            )}
            <button className="danger" type="button" onClick={() => props.onDecide(approval.toolCallId, "deny")}>
              Deny
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}

function EventBars(props: { rows: Array<[string, number]> }): React.JSX.Element {
  if (props.rows.length === 0) {
    return (
      <div className="empty-state-block">
        <strong>No events captured</strong>
        <span>Event distribution appears after the first run writes to the ledger.</span>
      </div>
    );
  }

  const max = Math.max(...props.rows.map(([, count]) => count));
  return (
    <div className="bar-chart-css">
      {props.rows.map(([kind, count]) => (
        <div className="bar-row" key={kind}>
          <span className="bar-label">{kind}</span>
          <div className="bar-container">
            <div className="bar-fill" style={{ width: `${Math.max(6, (count / max) * 100)}%` }} />
          </div>
          <span className="bar-count">{count}</span>
        </div>
      ))}
    </div>
  );
}

function SignalSummary(props: { toolErrors: number; guardVerdicts: number }): React.JSX.Element {
  return (
    <div className="signal-list">
      <div>
        <span>Tool errors</span>
        <strong className={props.toolErrors > 0 ? "danger-text" : "ok-text"}>{props.toolErrors}</strong>
      </div>
      <div>
        <span>Guard verdicts</span>
        <strong>{props.guardVerdicts}</strong>
      </div>
      <div>
        <span>Replay posture</span>
        <strong>{props.guardVerdicts > 0 ? "Auditable" : "Ready"}</strong>
      </div>
    </div>
  );
}

function SessionMessage(props: {
  expanded: boolean;
  message: ChatMessage;
  onDecide: (toolCallId: string, kind: "approve" | "approve_constrained" | "deny", hintEvent?: ActantEvent) => Promise<void>;
  onToggle: () => void;
}): React.JSX.Element {
  const pendingApproval = props.message.pendingApproval;

  return (
    <article className={`session-message ${props.message.role}`}>
      <header>
        <strong>{props.message.role}</strong>
        <span>{props.message.timestamp}</span>
      </header>
      <p>{props.message.content}</p>
      {pendingApproval && (
        <div className="approval-item inline">
          <div>
            <strong>{pendingApproval.request.tool}</strong>
            <span>{compactJson(pendingApproval.request.args)}</span>
          </div>
          <div className="approval-actions">
            <button
              type="button"
              onClick={() => void props.onDecide(pendingApproval.toolCallId, "approve")}
            >
              Approve
            </button>
            <button
              className="danger"
              type="button"
              onClick={() => void props.onDecide(pendingApproval.toolCallId, "deny")}
            >
              Deny
            </button>
          </div>
        </div>
      )}
      {props.message.subEvents.length > 0 && (
        <div className="message-events">
          <button className="secondary" type="button" onClick={props.onToggle}>
            {props.expanded ? "Hide" : "Show"} {props.message.subEvents.length} ledger rows
          </button>
          {props.expanded && (
            <div>
              {props.message.subEvents.map((event) => (
                <div key={event.id} className="message-event-row">
                  <span>{event.kind}</span>
                  <code>{eventSummary(event)}</code>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </article>
  );
}

function buildDecision(
  kind: "approve" | "approve_constrained" | "deny",
  hintEvent?: ActantEvent,
): ApprovalDecision {
  if (kind === "approve") {
    return { decision: "approve", approver: "studio", scope: "once" };
  }
  if (kind === "approve_constrained") {
    const payload = hintEvent ? payloadRecord(hintEvent) : {};
    return {
      decision: "approve_constrained",
      approver: "studio",
      scope: "once",
      accepted_input: payload.constrained_input ?? payload.args ?? null,
    };
  }
  return { decision: "deny", approver: "studio", reason: "denied from Studio" };
}

function buildChatMessages(events: ActantEvent[], approvals: ApprovalRecord[]): ChatMessage[] {
  const messages: ChatMessage[] = [];
  let currentMessage: ChatMessage | null = null;

  for (const event of events) {
    const payload = payloadRecord(event);
    if (event.kind === "user_message_received") {
      currentMessage = {
        id: event.id,
        role: "user",
        content: valueText(payload.text),
        timestamp: event.created_at.slice(11, 19),
        rawEvent: event,
        subEvents: [],
      };
      messages.push(currentMessage);
      continue;
    }

    if (event.kind === "agent_run_finished") {
      messages.push({
        id: event.id,
        role: "system",
        content: truthy(payload.ok) ? "Task finished successfully." : "Task finished with failure.",
        timestamp: event.created_at.slice(11, 19),
        rawEvent: event,
        subEvents: [],
      });
      continue;
    }

    if (event.kind === "approval_required") {
      const toolCallId = valueText(payload.tool_call_id);
      messages.push({
        id: event.id,
        role: "system",
        content: `${valueText(payload.tool)} requires approval.`,
        timestamp: event.created_at.slice(11, 19),
        rawEvent: event,
        subEvents: [],
        pendingApproval: approvals.find((approval) => approval.toolCallId === toolCallId),
      });
      continue;
    }

    if (currentMessage) {
      currentMessage.subEvents.push(event);
    } else {
      messages.push({
        id: event.id,
        role: "system",
        content: `${event.kind}: ${compactJson(event.payload)}`,
        timestamp: event.created_at.slice(11, 19),
        rawEvent: event,
        subEvents: [],
      });
    }
  }

  return messages;
}

function getDbTableRows(
  table: DbTable,
  query: string,
  runs: StudioInfo["runs"],
  events: ActantEvent[],
  approvals: ApprovalRecord[],
): TableRow[] {
  const needle = query.toLowerCase();
  if (table === "runs") {
    return runs
      .filter((run) => includes(run.runId, needle) || includes(run.startedAt, needle))
      .map((run) => ({
        id: run.runId,
        cells: [run.runId, String(run.events), dateText(run.startedAt)],
      }));
  }

  if (table === "events") {
    return events
      .filter((event) => includes(event.id, needle) || includes(event.kind, needle) || includes(event.run_id, needle))
      .map((event) => ({
        id: event.id,
        cells: [
          event.id,
          `${event.run_id.slice(0, 8)}…`,
          event.kind,
          event.created_at.slice(11, 19),
          compactJson(event.payload).slice(0, 90),
        ],
      }));
  }

  if (table === "approvals") {
    return approvals
      .filter((approval) => includes(approval.toolCallId, needle) || includes(approval.status, needle))
      .map((approval) => ({
        id: approval.toolCallId,
        cells: [
          approval.toolCallId,
          `${approval.runId.slice(0, 8)}…`,
          approval.status,
          approval.request.tool,
          approval.createdAt.slice(11, 19),
        ],
      }));
  }

  const actorCounts: Record<string, { runIds: Set<string>; events: number }> = {};
  for (const event of events) {
    const payload = payloadRecord(event);
    const actor = valueText(payload.actor ?? payload.role ?? roleFallback(event.kind));
    actorCounts[actor] ??= { runIds: new Set<string>(), events: 0 };
    actorCounts[actor].runIds.add(event.run_id);
    actorCounts[actor].events += 1;
  }

  return Object.entries(actorCounts)
    .filter(([actor]) => includes(actor, needle))
    .map(([actor, stats]) => ({
      id: actor,
      cells: [actor, String(stats.runIds.size), String(stats.events)],
    }));
}

function countByKind(events: ActantEvent[]): Record<string, number> {
  return events.reduce<Record<string, number>>((counts, event) => {
    counts[event.kind] = (counts[event.kind] ?? 0) + 1;
    return counts;
  }, {});
}

function payloadRecord(event: ActantEvent): PayloadRecord {
  return isPayloadRecord(event.payload) ? event.payload : {};
}

function eventSummary(event: ActantEvent): string {
  const payload = payloadRecord(event);
  if (event.kind === "model_call") return `${valueText(payload.role)} ${valueText(payload.summary)}`;
  if (event.kind === "tool_call_requested") return `${valueText(payload.tool)} ${compactJson(payload.args)}`;
  if (event.kind === "tool_call_completed") return `${valueText(payload.status)} ${valueText(payload.duration_ms)}ms`;
  if (event.kind === "guard_verdict") return `${valueText(payload.decision)} ${valueText(payload.reason)}`;
  if (event.kind === "context_build") return `${arrayLength(payload.included)} included, ${arrayLength(payload.blocked)} blocked`;
  if (event.kind === "agent_run_started") return "run started";
  return compactJson(event.payload);
}

function compactJson<T>(value: T | undefined): string {
  if (value === undefined) return "";
  try {
    return JSON.stringify(value) ?? "";
  } catch {
    return String(value);
  }
}

function valueText(value: JsonValue | undefined): string {
  if (value === undefined || value === null) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return compactJson(value);
}

function arrayLength(value: JsonValue | undefined): number {
  return Array.isArray(value) ? value.length : 0;
}

function truthy(value: JsonValue | undefined): boolean {
  return value === true || value === "true";
}

function includes(value: string, needle: string): boolean {
  return needle.length === 0 || value.toLowerCase().includes(needle);
}

function dateText(value: string): string {
  return value.slice(0, 19).replace("T", " ");
}

function shortPath(path: string): string {
  if (path.length <= 34) return path;
  return `…${path.slice(-33)}`;
}

function roleFallback(kind: string): string {
  return kind === "user_message_received" ? "user" : "system";
}

function featureHint(tab: TabId): string {
  if (tab === "overview") return "health, activity, shortcuts";
  if (tab === "tables") return "runs, events, approvals";
  if (tab === "sql") return "saved ledger queries";
  if (tab === "database") return "schema and chain health";
  if (tab === "api") return "SDK and local endpoints";
  if (tab === "auth") return "guards and approvals";
  if (tab === "storage") return "local store and artifacts";
  if (tab === "realtime") return "active event stream";
  if (tab === "functions") return "runs and replay";
  if (tab === "logs") return "event explorer";
  if (tab === "reports") return "metrics and usage";
  if (tab === "backups") return "snapshot posture";
  if (tab === "branches") return "environment context";
  return "project configuration";
}

function statusClass(status: FeatureStatus): string {
  if (status === "Available") return "available";
  if (status === "Read-only") return "readonly";
  return "configure";
}

function isPayloadRecord(value: ActantEvent["payload"]): value is PayloadRecord {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  return Object.values(value).every(isJsonValue);
}

function isJsonValue(value: ActantEvent["payload"]): value is JsonValue {
  if (value === null) return true;
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return true;
  if (Array.isArray(value)) return value.every(isJsonValue);
  return isPayloadRecord(value);
}

function tabTitle(tab: TabId): string {
  if (tab === "overview") return "Project Home";
  if (tab === "tables") return "Table Editor";
  if (tab === "sql") return "SQL Editor";
  if (tab === "database") return "Database";
  if (tab === "api") return "API Docs";
  if (tab === "auth") return "Authentication";
  if (tab === "storage") return "Storage";
  if (tab === "realtime") return "Realtime";
  if (tab === "functions") return "Edge Functions";
  if (tab === "logs") return "Logs Explorer";
  if (tab === "reports") return "Reports";
  if (tab === "backups") return "Backups";
  if (tab === "branches") return "Branches";
  return "Project Settings";
}

function errorMessage<T>(err: T): string {
  return err instanceof Error ? err.message : String(err);
}
