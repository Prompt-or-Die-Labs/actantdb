# actant-demo-cli

**Third public example — wedge wrapper on a hand-rolled CLI agent.**

No framework. Forty lines of plain JS. The wrapper still captures every tool call, runs Guard, and feeds Studio.

## Run it

```bash
pnpm install
node demo.mjs
npx actantdb studio --project demo-cli --store-dir ./.actantdb
```

You'll see Studio render the three planned tool calls — including the one Guard constrained (`rm -rf build dist` → `rm -rf build`).

## What this proves

The wedge isn't bound to any framework. A `cli-agent` with three tools and a `for` loop is enough. Useful for: codemods, ops scripts, anything you'd run in a terminal that calls tools.
