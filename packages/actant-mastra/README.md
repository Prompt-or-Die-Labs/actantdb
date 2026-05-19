# @actantdb/mastra

Wrap a Mastra agent. Capture every model call, tool call, context manifest, approval, and effect result. Replay from any decision point.

You keep Mastra. You add Actant.

## Install

```bash
npm install @actantdb/mastra
```

## Use

```ts
import { withActant } from "@actantdb/mastra";
import { myMastraAgent } from "./agent";

export const agent = withActant(myMastraAgent, {
  project: "prod-support-agent",
  replay: true,
  approvals: true,
});
```

Then open Studio:

```bash
npx actant studio
```

## What gets captured

| Event                       | What it records                                      |
| --------------------------- | ---------------------------------------------------- |
| `agent.run.started`         | run id, agent name, initiating message               |
| `model.call`                | model + provider + redacted prompt hash + response hash + tokens + latency |
| `tool.call.requested`       | tool name + arguments (with secret redaction) + risk class |
| `guard.verdict`             | allow / constrain / require_approval / block / halt + policy snapshot |
| `approval.required`         | approval request id + summary + reversibility       |
| `approval.decision`         | approver + scope (once / session / etc) + decision  |
| `tool.call.completed`       | result ref + duration + exit / status               |
| `context.build`             | manifest hash + included / blocked counts + per-item visibility decision |
| `effect.observed`           | structured observation (when the tool emits one)     |

Captured locally to `~/.actant/<project>/events.sqlite`. No remote backend required. No data leaves your machine unless you opt in.

## Approval API

In Studio: click the pending approval, choose **allow**, **constrain** (rewrites the arguments), or **deny**.

Or from the CLI:

```bash
actant approve <tool_call_id>
actant approve <tool_call_id> --scope session
actant deny    <tool_call_id> --reason "policy mismatch"
```

## Replay

```bash
actant replay run <event_id> --without-memory mem_42
actant replay run <event_id> --policy strict
actant replay diff <run_a> <run_b>
```

Replay does NOT re-execute real side effects in v0.1. Recorded tool results are reused. To re-invoke models or tools, opt into experimental mode (v0.2).

## Status

Pre-alpha. See [`/PIVOT.md`](../../PIVOT.md) for the project's current shape, and [`/CHANGELOG.md`](../../CHANGELOG.md) for the day-by-day milestone.

The Gate-1 target is **2026-06-30**: this package wraps a Mastra agent, Studio renders the timeline, approval works, basic replay checkpoint works, 3 non-Wes developers have run it on a real agent.

## License

Apache 2.0.
