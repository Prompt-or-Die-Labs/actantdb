# Golden Quickstart

This is the first path. It stays local, uses the embedded SQLite ledger, and
does not require a Rust server, Docker, a hosted service, or a model API key.

Requirements:

- Node 24 recommended.
- Node 22.5 or newer works when `node:sqlite` is available in your runtime.

```bash
npm create actantdb@latest my-agent -- --template minimal --framework hand-rolled --language js --yes
cd my-agent
npm install
npm start
npm run studio
npm run doctor
```

Expected result:

- `npm start` records one hash-chained run under `./.actantdb`.
- `npm run studio` opens the local timeline for `my-agent`.
- `npm run doctor` checks the ledger file and schema.

The scaffold is intentionally small:

```text
my-agent/
  package.json
  README.md
  agent.mjs
  .gitignore
  .actantdb/       # created after npm start
```

The generated agent does three things:

1. Wraps a no-op agent with `withActant()`.
2. Records a user message and completed run into the embedded ledger.
3. Prints the exact Studio command for the run it created.

After the first run, replace the no-op `generate` function and add tools to the
`tools` record. Keep the wrapper and the ledger stays replayable.

## Copy Into An Existing Agent

```js
import { withActant } from "@actantdb/mastra";

const wrapped = withActant(agent, {
  project: "my-agent",
  storeDir: "./.actantdb",
  autoApprove: true,
});

const run = wrapped.startRun({ meta: { source: "quickstart" } });
run.recordUserMessage("Hello ActantDB");
run.finish({ ok: true });
wrapped.actant.close();
```

Open it:

```bash
npx actantdb studio --project my-agent --store-dir ./.actantdb --port 4173
```

## Common First-Run Errors

| Error | Fix |
| --- | --- |
| `project name is required` | Pass a name: `npm create actantdb@latest my-agent -- --yes`. |
| `unknown option` | Run `npm create actantdb@latest -- --help`. |
| `unknown template` | Use `--template minimal` for the smallest first run. |
| `node:sqlite` unavailable | Use Node 24, or Node 22.5+ with a runtime that enables `node:sqlite`. |

