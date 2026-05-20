# actant-studio

Local operator UI + CLI for ActantDB. Ships the `actantdb` binary.

```bash
npx actantdb studio                                # open the local UI
actantdb approve <tool_call_id> --scope once
actantdb deny    <tool_call_id> --reason "out of policy"
actantdb replay run <event_id> --without-memory mem_42
actantdb replay diff <run_a> <run_b>
```

## What Studio shows

Per [`/examples/test-cleanup/README.md`](../../examples/test-cleanup/README.md):

- **Timeline.** Every captured event: model call, tool call requested, guard verdict, approval, tool result, context build, observation.
- **Context manifest detail.** What was included, what was blocked, why.
- **Approval drawer.** Approve / deny / constrain a pending tool call. The constrained variant is recorded.
- **Replay control.** Pick an event, choose overrides (policy, memory exclusion, alternate model), run, see the side-by-side diff.

That is the scope for the local backend console. Studio inspects and controls
agent backend state; it does not author or run agents.

## Tech

- React 19 + TypeScript strict.
- Vite for the UI bundle.
- Local Node HTTP server hosts the bundle + a websocket to `@actantdb/core`'s ledger.
- No remote backend. No telemetry leaving the machine by default.

## Status

Pre-1.0. The Studio surface is the visible part of the backend: ledger
timeline, approval queue, replay diffs, and diagnostics. If those records are
confusing in Studio, the backend contract is confusing.

See [`/PIVOT.md`](../../PIVOT.md), [`/CHANGELOG.md`](../../CHANGELOG.md), [`/CHANGELOG.md`](../../CHANGELOG.md).
