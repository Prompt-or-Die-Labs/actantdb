const steps = [
  {
    title: "Capture run",
    status: "recording",
    next: "Review authority",
    tab: "context",
    events: [
      ["run_started", "Run opened", "project=my-agent source=quickstart"],
      ["user_message", "User asks for cleanup", "Clean up the test artifacts."],
      ["context_build", "Context manifest attached", "README included; dist note included; secrets blocked."],
      ["model_call", "Planner proposes a shell tool", "shell.run rm -rf build dist"],
      ["guard_verdict", "Authority gate pauses the run", "decision=require_approval scope=constrain"],
    ],
  },
  {
    title: "Review authority",
    status: "paused",
    next: "Replay safely",
    tab: "guard",
    events: [
      ["run_started", "Run opened", "project=my-agent source=quickstart"],
      ["context_build", "Context manifest attached", "3 included, 1 blocked before model call."],
      ["model_call", "Planner proposes a shell tool", "shell.run rm -rf build dist"],
      ["guard_verdict", "Guard narrows the command", "remove dist; approve rm -rf build only."],
      ["approval_decision", "Approval is recorded", "approver=local-reviewer scope=once"],
      ["tool_call", "Tool executes constrained action", "shell.run rm -rf build"],
      ["effect_completed", "Result is chained", "exit=0 stdout=clean"],
    ],
  },
  {
    title: "Replay safely",
    status: "replayed",
    next: "Capture run",
    tab: "replay",
    events: [
      ["replay_started", "Replay forks from model decision", "override: exclude memory mem_42_dist"],
      ["context_build", "Manifest changes", "2 included, 1 blocked by override."],
      ["model_call", "Planner chooses safe command", "shell.run rm -rf build"],
      ["guard_verdict", "No approval needed", "decision=allow"],
      ["replay_diff", "Causal diff is explicit", "stale memory caused the risky proposal."],
    ],
  },
];

const details = {
  context: `
    <h3>Context manifest</h3>
    <dl>
      <dt>Included</dt><dd>README.md, package.json, previous run summary</dd>
      <dt>Blocked</dt><dd>local-only secret note</dd>
      <dt>Why it matters</dt><dd>The ledger stores what the model could see before the tool call.</dd>
      <dt>Hash chain</dt><dd>Every row points to the previous row hash.</dd>
    </dl>
    <div class="tag-row">
      <span class="tag ok">public README</span>
      <span class="tag ok">low sensitivity</span>
      <span class="tag danger">secret blocked</span>
    </div>
  `,
  guard: `
    <h3>Authority decision</h3>
    <dl>
      <dt>Requested</dt><dd><span class="danger">rm -rf build dist</span></dd>
      <dt>Policy</dt><dd>Constrain destructive shell calls to the named build directory.</dd>
      <dt>Decision</dt><dd><span class="warn">require approval</span></dd>
      <dt>Approved</dt><dd><span class="ok">rm -rf build</span></dd>
    </dl>
    <div class="tag-row">
      <span class="tag">guard_verdict</span>
      <span class="tag">approval_decision</span>
      <span class="tag">tool_call</span>
    </div>
  `,
  replay: `
    <h3>Replay diff</h3>
    <ol class="diff">
      <li><strong>Context</strong><span>Memory <code>mem_42_dist</code> is excluded.</span></li>
      <li><strong>Model</strong><span>Planner asks for <code>rm -rf build</code> directly.</span></li>
      <li><strong>Guard</strong><span>Decision changes from <span class="warn">constrain</span> to <span class="ok">allow</span>.</span></li>
      <li><strong>Proof</strong><span>The stale memory caused the risky tool proposal.</span></li>
    </ol>
  `,
};

let activeStep = 0;
let activeTab = "context";

const eventList = document.querySelector("#event-list");
const title = document.querySelector("#step-title");
const status = document.querySelector("#run-status");
const advance = document.querySelector("#advance");
const detailBody = document.querySelector("#detail-body");
const stepButtons = Array.from(document.querySelectorAll(".step"));
const tabButtons = Array.from(document.querySelectorAll("[role='tab']"));
const quickstart = document.querySelector("#quickstart");

function render() {
  const step = steps[activeStep];
  activeTab = step.tab;
  title.textContent = step.title;
  status.textContent = step.status;
  advance.textContent = step.next;
  eventList.innerHTML = step.events
    .map((event, index) => eventRow(event, index))
    .join("");
  stepButtons.forEach((button, index) => {
    button.classList.toggle("is-active", index === activeStep);
  });
  renderTabs();
}

function eventRow([kind, heading, summary], index) {
  const hash = `h${String(index + 1).padStart(2, "0")}_${hashText(kind + heading).slice(0, 6)}`;
  return `
    <li class="event">
      <span class="kind">${kind}</span>
      <span class="summary">
        <strong>${heading}</strong>
        <p>${summary}</p>
      </span>
      <span class="hash">${hash}</span>
    </li>
  `;
}

function hashText(input) {
  let out = 0;
  for (const char of input) out = (out * 31 + char.charCodeAt(0)) >>> 0;
  return out.toString(16);
}

function renderTabs() {
  tabButtons.forEach((button) => {
    const selected = button.dataset.tab === activeTab;
    button.setAttribute("aria-selected", String(selected));
  });
  detailBody.innerHTML = details[activeTab];
}

stepButtons.forEach((button, index) => {
  button.addEventListener("click", () => {
    activeStep = index;
    render();
  });
});

tabButtons.forEach((button) => {
  button.addEventListener("click", () => {
    activeTab = button.dataset.tab;
    renderTabs();
  });
});

advance.addEventListener("click", () => {
  activeStep = activeStep === steps.length - 1 ? 0 : activeStep + 1;
  render();
});

document.querySelector("#reset").addEventListener("click", () => {
  activeStep = 0;
  render();
});

document.querySelector("#copy-command").addEventListener("click", async (event) => {
  const button = event.currentTarget;
  try {
    await navigator.clipboard.writeText(quickstart.textContent);
    button.textContent = "Copied";
  } catch {
    button.textContent = "Select command";
  }
  window.setTimeout(() => {
    button.textContent = "Copy";
  }, 1400);
});

render();
