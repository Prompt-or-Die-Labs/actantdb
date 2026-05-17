# Actant Studio — design

Studio is the dashboard. It is a single TypeScript / React app under `studio/` that subscribes to ActantDB via the WebSocket API. Phase 1 ships a minimal Studio; Phases 2 → 6 add screens.

## Tech stack

- React 19 + TypeScript (strict).
- Server state via the generated TypeScript SDK (`sdks/ts`).
- Routing: TanStack Router.
- Styling: Tailwind + shadcn/ui.
- No state framework beyond React + the SDK's subscription hooks.
- Build: Vite.

## Screen catalog by phase

### Phase 1 (minimal)

| Screen           | Subscribes to                       | Actions                                |
| ---------------- | ----------------------------------- | -------------------------------------- |
| **Chat**         | `message`, `agent_event`            | `append_user_message`                  |
| **Approval Center** | `approval_request status=pending`| `approve_tool_call`, `deny_tool_call`  |
| **Audit Trail**  | `agent_event`                       | (read only)                            |
| **Memory Review** | `memory_candidate status=pending`  | `approve_memory`, `reject_memory`      |

### Phase 2 additions

| Screen              | Subscribes to                      |
| ------------------- | ---------------------------------- |
| **Workers**         | `worker`, `worker_heartbeat`       |
| **Effect Queue**    | `effect`                           |
| **Intent Inspector**| `intent`                           |
| **Drift Monitor**   | `drift_signal`                     |
| **Intervention log**| `intervention`                     |

Approval Center gains a compensation-plan badge ("revertible / partial / irreversible") and a redacted-input preview.

### Phase 3 additions

| Screen             | Subscribes to                      |
| ------------------ | ---------------------------------- |
| **Context Inspector** | `context_build`, `context_item` |
| **Memory detail**  | `memory`, `memory_use`, `memory_candidate`, `memory_conflict` |
| **Capsule Browser**| `capsule`, `capsule_membership`    |
| **Trust panel**    | `trust_profile`                    |

### Phase 4 additions

| Screen               | Subscribes to                          |
| -------------------- | -------------------------------------- |
| **Workflow Board**   | `workflow_run`, `workflow_step_run`    |
| **Workflow Timeline**| (per-run, joins `workflow_step_run`, `agent_event`) |
| **Trigger Manager**  | `trigger`                              |
| **Regret Inbox**     | `regret_event`                         |
| **Eval Catalog**     | `eval_case`, `eval_run`                |
| **Agent Tasks**      | `agent_task`                           |

### Phase 5 additions

| Screen           | Subscribes to                  |
| ---------------- | ------------------------------ |
| **Replay Lab**   | `replay_run`                   |
| **Diff Viewer**  | `replay_diff` (filtered by run)|

### Phase 6 additions

| Screen                  | Subscribes to                          |
| ----------------------- | -------------------------------------- |
| **Workspace switcher**  | `workspace`                            |
| **Members + roles**     | `actor`, `authority_scope`             |
| **Quotas**              | per-workspace quota table              |
| **Audit Explorer**      | full-text + structured over `agent_event` |
| **Retention manager**   | per-workspace retention configuration  |
| **Sync settings**       | `capsule`, sync targets                |

## Interaction patterns

- **All writes go through commands.** Studio never writes directly; it issues commands via the SDK.
- **Snapshot-then-stream.** Every screen receives an initial snapshot of matching rows, then incremental updates over the same subscription.
- **Optimistic UI is prohibited.** Studio renders only committed state. Users see the round-trip latency; that latency *is* the system's correctness budget.
- **Approvals are deliberate.** High-risk approvals require a typed confirmation phrase ("approve send" for `email.send`, etc.).
- **Replay is everywhere.** From any chronicle event the user can launch a replay rooted at that event's checkpoint.

## Accessibility

- WCAG 2.1 AA target.
- Color is not the only signal for risk levels (icons + text).
- All approval flows reachable from keyboard alone.
- Live regions for streaming approval notifications.

## Performance budgets

- First meaningful paint < 2s on a cold session with a 500-row Approval Center.
- Subscription update → DOM render < 200ms p99.
- 5k-event Audit Trail render < 1s with virtualization.

## Studio packaging

- Phase 1: bundled inside `actantdb-server` (served from `/` as static assets).
- Phase 6: optional separate deploy (`actant-studio` Docker image / Vercel app) pointing at a remote `actantdb-server`.

## What Studio is NOT

- An IDE.
- A model playground.
- A general-purpose admin console.

It is a **control surface for accountable agents**. Every screen exists because some operation in the Actant Contract needs visible state.
