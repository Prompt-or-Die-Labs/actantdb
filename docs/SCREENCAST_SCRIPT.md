# 90-second ActantDB screencast — script + storyboard

Closes the planning half of [GAPS.md row #10](../GAPS.md). Recording is the
only remaining human step.

## Setup (do once, off-camera)

- macOS, terminal at 14pt or larger, dark mode, font with ligatures off so
  every character is unambiguous.
- Fresh `node ≥ 22.5`, `pnpm` installed.
- `mkdir /tmp/screencast && cd /tmp/screencast`
- Screen recorder rolling at 1920×1080. Audio: voiceover or none — the
  script reads top-to-bottom either way.

## Timeline

```
0:00 — 0:05   Title card: "ActantDB — every agent action, replayable."
0:05 — 0:20   Install
0:20 — 0:35   Capture a run
0:35 — 0:50   Open Studio
0:50 — 1:10   Replay with override
1:10 — 1:25   Show the diff
1:25 — 1:30   End card: "@actantdb/all — `npm install` to ship."
```

## Cue-by-cue

### 0:00 — 0:05  Title card

Static frame.

> **ActantDB — every agent action, replayable.**
> *npm install @actantdb/all*

### 0:05 — 0:20  Install (15s)

Type, run, wait for the success line.

```bash
npm init -y
npm install @actantdb/all
```

Voiceover: *"One install. Storage, policy, replay, agents, box — every
ActantDB primitive in a single dependency."*

### 0:20 — 0:35  Capture a run (15s)

```bash
node -e "
import { Box, Agent, ClaudeCode } from '@actantdb/all';
const box = await Box.create({ name: 'demo' });
const run = await box.agent.run({ prompt: 'list workspace files' });
console.log(run.result);
"
```

Point at the captured `tool_call_*` events scrolling. Voiceover: *"Every
exec, every file write, every model call lands as a typed event in a
hash-chained ledger."*

### 0:35 — 0:50  Open Studio (15s)

```bash
npx actantdb studio --project demo
```

Browser opens automatically. Hover on the run timeline. Click a
`tool_call_completed` event. Right pane shows the manifest + the policy
verdict.

Voiceover: *"Studio renders the timeline, the model context, and every
Guard decision side by side."*

### 0:50 — 1:10  Replay with override (20s)

In the right pane, click "Replay". Pick `mode: tool` from the dropdown.
Paste a substitute result:

```json
{ "stub-tool": { "ok": false } }
```

Click "Run". The diff renders inline.

Voiceover: *"Replay isn't disk restore — it's causal re-run. Change a
policy, drop a memory item, substitute a tool result. See exactly what
would have happened."*

### 1:10 — 1:25  The diff (15s)

Zoom on the side-by-side diff. The substituted row is highlighted
"changed"; everything downstream is highlighted too. Show the manifest
hash difference.

Voiceover: *"Hash-chained, so the diff is exact. The substrate is the
proof."*

### 1:25 — 1:30  End card (5s)

Static frame.

> **`npm install @actantdb/all`**
> *github.com/Prompt-or-Die-Labs/actantdb*

## Asset list (post-recording)

- `hero.png` — frame from 0:50 (Studio + the right pane with policy
  verdict). Crop to 1600×900.
- `screencast.mp4` — full 90-second file. Drop into `examples/test-cleanup/`
  next to the existing `killer-demo.cast`.

## What this script proves

- Single import for everything (`@actantdb/all`).
- Agent capture without writing glue code (`Box.create` + harness).
- Studio renders local-first (browser auto-opens).
- Replay-with-override is causal, not snapshot.

Stay under 90 seconds; cut anything that doesn't earn its place.
