# create-actantdb

Scaffold a new ActantDB project. One command, no boilerplate hunting.

## Usage

```bash
# Interactive — pick template, framework, language:
npm create actantdb@latest my-app

# Golden local path — embedded ledger, JavaScript, no server:
npm create actantdb@latest my-app -- --template minimal --framework hand-rolled --language js --yes

# Bun first-run path:
npm create actantdb@latest my-app -- --template minimal --framework hand-rolled --language js --runtime bun --yes

# Non-interactive TypeScript path:
npx create-actantdb my-app \
  --template coding-agent \
  --framework mastra \
  --language ts \
  --yes
```

After scaffolding:

```bash
cd my-app
npm install
npm start          # runs the agent stub, records a sample run
npm run studio     # opens the Studio timeline
npm run doctor     # checks the embedded ledger
```

## Templates

| Template          | Description |
|-------------------|-------------|
| `minimal`         | Embedded ledger + `withActant()` wrapper, no real agent. |
| `coding-agent`    | Mastra coding agent with replay-able tool calls + approval gates. |
| `research-agent`  | Multi-step research with durable workflows, retries, approvals. |
| `support-agent`   | Customer-support agent with reviewable memory + replay-on-complaint. |
| `fanout-agent`    | Spawn parallel sub-agents, aggregate, gate with Guard verdicts. |

## Frameworks

`--framework` accepts `mastra | langgraph | vercel-ai | openai-agents | hand-rolled`.
The wrapper is framework-agnostic; the flag picks the right peer dep
and the right stub agent shape inside the scaffolded project.

## Flags

| Flag                | Default        | Effect |
|---------------------|----------------|--------|
| `--template`        | (prompt)       | Template id. |
| `--framework`       | template default | Framework id. |
| `--language`        | `ts`           | `ts` or `js`. |
| `--runtime`         | `node`         | `node` or `bun`; controls the scaffolded start script and engines field. |
| `--port`            | `4173`         | Studio port wired into the `npm run studio` script. |
| `--no-interactive`  | off            | Skip prompts; require every choice. |
| `--yes`, `-y`       | off            | Alias for `--no-interactive`. |
| `--force`           | off            | Allow scaffolding into a non-empty directory. |

## What gets written

```
my-app/
├── package.json           # @actantdb/* deps already wired
├── tsconfig.json          # (when --language ts)
├── README.md              # next steps
├── .gitignore
├── src/agent.ts           # (or agent.mjs for --language js)
└── .actantdb/             # created on first run; the SQLite ledger
```

The scaffolded `package.json` pins `@actantdb/*` at the same major/minor
as the create-actantdb that produced it, so versions stay aligned.
