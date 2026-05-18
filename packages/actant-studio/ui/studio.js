// Actant Studio UI — vanilla JS, no build step. Talks to /api/* on the same origin.

const state = {
  project: "",
  runs: [],
  activeRun: null,
  events: [],
  selected: null,
  replayResult: null,
};

async function fetchJSON(url, opts) {
  const r = await fetch(url, opts);
  if (!r.ok) throw new Error(`HTTP ${r.status} for ${url}`);
  return r.json();
}

async function refresh() {
  const info = await fetchJSON("/api/info");
  state.project = info.project;
  state.runs = info.runs ?? [];
  document.getElementById("meta").textContent = `${info.project} · ${info.dbPath}`;
  renderRuns();
  if (!state.activeRun && state.runs.length > 0) {
    state.activeRun = state.runs[state.runs.length - 1].runId;
  }
  if (state.activeRun) {
    const r = await fetchJSON(`/api/events?run=${encodeURIComponent(state.activeRun)}`);
    state.events = r.events ?? [];
    state.replayResult = null;
    renderEvents();
  }
}

function renderRuns() {
  const ul = document.getElementById("runs-list");
  ul.innerHTML = "";
  for (const r of state.runs) {
    const li = document.createElement("li");
    li.className = r.runId === state.activeRun ? "active" : "";
    li.innerHTML = `<div>${r.runId.slice(0, 10)}…</div><small>${r.events} events · ${r.startedAt}</small>`;
    li.onclick = async () => {
      state.activeRun = r.runId;
      await refresh();
    };
    ul.appendChild(li);
  }
}

function renderEvents() {
  const root = document.getElementById("events");
  root.innerHTML = "";
  for (const e of state.events) {
    const row = document.createElement("div");
    row.className = "event" + (state.selected?.id === e.id ? " selected" : "");
    row.onclick = () => selectEvent(e);
    const ts = e.created_at.slice(11, 19);
    const summary = summarize(e);
    const badge = badgeFor(e);
    row.innerHTML = `
      <div class="ts">${ts}</div>
      <div class="kind">${e.kind}</div>
      <div class="summary">${escapeHTML(summary)}</div>
      <div class="badge ${badge.cls}">${badge.text}</div>
    `;
    root.appendChild(row);
  }
}

function summarize(e) {
  switch (e.kind) {
    case "user_message_received":
      return JSON.stringify(e.payload.text);
    case "model_call":
      return `${e.payload.role}: ${e.payload.summary}`;
    case "context_build": {
      const inc = e.payload.included?.length ?? 0;
      const blk = e.payload.blocked?.length ?? 0;
      return `${inc} included, ${blk} blocked`;
    }
    case "tool_call_requested":
      return `${e.payload.tool} ${JSON.stringify(e.payload.args)}`;
    case "tool_call_started":
      return `${JSON.stringify(e.payload.final_args)}`;
    case "tool_call_completed":
      return `status=${e.payload.status} ${e.payload.duration_ms}ms`;
    case "guard_verdict":
      return `${e.payload.decision} — ${e.payload.reason}`;
    case "approval_required":
      return `${e.payload.tool} ${e.payload.hint ? "hint: " + e.payload.hint : ""}`;
    case "approval_decision":
      return `${e.payload.decision}${e.payload.approver ? " by " + e.payload.approver : ""}`;
    case "effect_observed":
      return JSON.stringify(e.payload);
    case "agent_run_finished":
      return JSON.stringify(e.payload);
    case "agent_run_started":
      return "run started";
    default:
      return "";
  }
}

function badgeFor(e) {
  if (e.kind === "guard_verdict") return { cls: "guard", text: e.payload.decision };
  if (e.kind === "approval_required") return { cls: "approval", text: "approval" };
  if (e.kind === "approval_decision") return { cls: "approval", text: e.payload.decision };
  if (e.kind === "tool_call_completed") {
    const s = e.payload.status;
    return { cls: s === "ok" ? "completed" : "blocked", text: s };
  }
  return { cls: "", text: "" };
}

function selectEvent(e) {
  state.selected = e;
  renderEvents();
  renderDetail(e);
}

function renderDetail(e) {
  const root = document.getElementById("detail-body");
  const json = JSON.stringify(e, null, 2);
  const actions = [];
  if (e.kind === "model_call" || e.kind === "context_build") {
    actions.push(`<button id="replay-btn">Replay from here</button>`);
  }
  if (e.kind === "approval_required") {
    actions.push(`<button id="approve-btn">Approve</button>`);
    actions.push(`<button id="approve-constrain-btn" class="secondary">Approve constrained</button>`);
    actions.push(`<button id="deny-btn" class="danger">Deny</button>`);
  }
  root.innerHTML = `<pre>${escapeHTML(json)}</pre>
    <div class="detail-actions">${actions.join(" ")}</div>`;
  document.getElementById("replay-btn")?.addEventListener("click", () => openReplay(e));
  document.getElementById("approve-btn")?.addEventListener("click", () => decideApproval(e, "approve"));
  document.getElementById("approve-constrain-btn")?.addEventListener("click", () => decideApproval(e, "approve_constrained"));
  document.getElementById("deny-btn")?.addEventListener("click", () => decideApproval(e, "deny"));
  if (state.replayResult) renderDiff(state.replayResult.diff);
}

async function decideApproval(e, kind) {
  const toolCallId = e.payload.tool_call_id;
  let decision;
  if (kind === "approve") decision = { decision: "approve", approver: "studio", scope: "once" };
  else if (kind === "approve_constrained")
    decision = {
      decision: "approve_constrained",
      approver: "studio",
      scope: "once",
      accepted_input: e.payload.constrained_input ?? e.payload.args,
    };
  else decision = { decision: "deny", approver: "studio", reason: "denied from Studio" };
  await fetch("/api/approvals/decide", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ toolCallId, decision }),
  });
  await refresh();
}

function openReplay(anchor) {
  document.getElementById("replay-anchor").textContent =
    `Anchor: ${anchor.kind} @ ${anchor.created_at} (${anchor.id})`;
  document.getElementById("opt-strict").checked = true;
  document.getElementById("opt-mem42").checked = true;
  document.getElementById("opt-mem-other").value = "";
  const dlg = document.getElementById("replay-modal");
  dlg.showModal();
  document.getElementById("run-replay").onclick = async () => {
    const useStrictPolicy = document.getElementById("opt-strict").checked;
    const mem42 = document.getElementById("opt-mem42").checked;
    const other = document.getElementById("opt-mem-other").value.trim();
    const without_memory = [];
    if (mem42) without_memory.push("mem_42_dist");
    if (other) without_memory.push(other);
    const mode =
      document.querySelector('input[name="mode"]:checked')?.value || "model";
    const body = {
      eventId: anchor.id,
      overrides: { without_memory },
      useStrictPolicy,
      mode,
    };
    const r = await fetchJSON("/api/replay", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    state.replayResult = r;
    dlg.close();
    renderDetail(anchor);
  };
  document.getElementById("cancel-replay").onclick = () => dlg.close();
}

function renderDiff(d) {
  const root = document.getElementById("detail-body");
  const rows = d.entries
    .map(
      (entry) => `<tr>
        <td>${entry.kind}</td>
        <td class="${entry.diff}">${entry.diff}</td>
        <td>${escapeHTML(JSON.stringify(entry.a ?? "", null, 0))}</td>
        <td>${escapeHTML(JSON.stringify(entry.b ?? "", null, 0))}</td>
      </tr>`,
    )
    .join("");
  const callout = `<div class="callout">Without the excluded memory, the planner proposed a safer command. Memory caused the risky proposal; Guard caught it; replay proves the link.</div>`;
  root.innerHTML += `
    <table class="diff-table">
      <thead><tr><th>event</th><th>diff</th><th>original</th><th>replay</th></tr></thead>
      <tbody>${rows}</tbody>
    </table>${callout}`;
}

function escapeHTML(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

refresh().catch((err) => {
  document.getElementById("events").textContent = `Error: ${err.message}`;
});

setInterval(() => {
  // Light auto-refresh while open
  refresh().catch(() => {});
}, 4000);
