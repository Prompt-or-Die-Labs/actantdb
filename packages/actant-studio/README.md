# actant-studio

Local UI + CLI for Actant. Ships the `actant` binary.

```bash
npx actant studio                                # open the local UI
actant approve <tool_call_id> --scope once
actant deny    <tool_call_id> --reason "out of policy"
actant replay run <event_id> --without-memory mem_42
actant replay diff <run_a> <run_b>
```

## What Studio shows

Per [`/wedge/killer-demo.md`](../../wedge/killer-demo.md):

- **Timeline.** Every captured event: model call, tool call requested, guard verdict, approval, tool result, context build, observation.
- **Context manifest detail.** What was included, what was blocked, why.
- **Approval drawer.** Approve / deny / constrain a pending tool call. The constrained variant is recorded.
- **Replay control.** Pick an event, choose overrides (policy, memory exclusion, alternate model), run, see the side-by-side diff.

That is the scope for v0.1. Anything more is post-Gate-3.

## Tech

- React 19 + TypeScript strict.
- Vite for the UI bundle.
- Local Node HTTP server hosts the bundle + a websocket to `@actantdb/core`'s ledger.
- No remote backend. No telemetry leaving the machine by default.

## Status

Pre-alpha. The Studio surface is the visible part of the killer demo and the validation tests. If the demo doesn't make sense in Studio, the demo is broken.

See [`/PIVOT.md`](../../PIVOT.md), [`/wedge/60-day-plan.md`](../../wedge/60-day-plan.md), [`/wedge/anti-scope.md`](../../wedge/anti-scope.md).
