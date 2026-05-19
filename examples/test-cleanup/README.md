# actant-demo-test-cleanup

Why did this agent delete the wrong file?

A Mastra-shaped test-cleanup agent wrapped through `@actantdb/mastra`. Its memory contains a stale fact ("build artifacts live under both /build and /dist"). It proposes `rm -rf build dist`. **Guard intercepts.**

## Run it

```bash
pnpm install
node demo.mjs                 # records the run
npx actantdb studio --project demo-test-cleanup --store-dir ./.actantdb
```

Open http://127.0.0.1:4555. Click the **model_call** row → **Replay from here**. Keep "stricter policy" and "exclude mem_42_dist" checked. Hit **Run replay**.

## What you see

```
event              original                       replay
─────────────────────────────────────────────────────────
context_build      3 included, 0 blocked          2 included, 1 blocked
model_call         rm -rf build dist              rm -rf build
guard_verdict      require_approval (constrain)   allow
tool_call          rm -rf build  (constrained)    rm -rf build
effect_completed   exit=0                         exit=0  (replayed)
```

Without the stale memory, the planner proposed the safe command directly. The memory caused the risky proposal; Guard caught it; replay proves the causal link.

## What this proves

- **Guard Authority** rewrote the destructive command before the shell ran.
- **Chronicle Replay** reran the planner under a different memory + policy.
- **Context manifests** made the model's input inspectable.
