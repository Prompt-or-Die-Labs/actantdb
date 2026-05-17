# Actant CLI — design

The CLI is a **first-class product surface**, not a project scaffolding sidecar. The developer promise:

> Developers can scaffold, run, inspect, replay, and ship an agentic backend faster than they can wire memory, tools, approvals, traces, and workflows by hand.

The CLI is the main interface for making autonomous agents understandable, testable, replayable, and shippable.

## Naming

| Binary             | Purpose                            |
| ------------------ | ---------------------------------- |
| `actant`           | The CLI. Everything is a subcommand. |
| `actantd`          | Optional daemon mode (Phase 4+).   |
| `actant studio`    | Studio launcher (browser open).    |
| `actantdb-server`  | The server binary (rarely run directly by devs once `actant` exists). |

Marketing names:

```
ActantDB        — the backend product
Actant CLI      — the developer tool
Actant Studio   — the dashboard
Actant SDK      — the language packages
Actant Workers  — the reference workers
```

## The first-run experience

```
brew install actantdb/tap/actant
# or
curl -fsSL https://actant.dev/install.sh | sh

actant
```

Output:

```
ActantDB — realtime backend for autonomous agents

No project detected.

Create a new project:
  actant new my-agent --template coding-agent

Try an example:
  actant examples run coding-agent

Start local Studio:
  actant studio
```

## The killer command: `actant dev`

```
actant dev
```

Does everything for local development in one command:

```
✓ Validate project config
✓ Start local ActantDB node
✓ Apply schema
✓ Start workers
✓ Start Studio
✓ Start example agent
✓ Tail live events
✓ Print URLs and next-step commands
```

Sample output:

```
ActantDB local node started

Database:
  actant://127.0.0.1:4739

Studio:
  http://127.0.0.1:4740

Workers:
  ✓ model_worker
  ✓ shell_worker
  ✓ file_worker

Subscriptions:
  ✓ approvals
  ✓ tool_calls
  ✓ memory_candidates
  ✓ workflow_runs

Try:
  actant chat
  actant workflow run fix_tests
  actant studio
```

## Top-level command map

```
# Project / lifecycle
actant new                Scaffold a new ActantDB project
actant init               Initialize ActantDB in an existing project
actant dev                Run local node, workers, Studio, examples
actant start              Start ActantDB node (no extras)
actant stop               Stop ActantDB node
actant status             Show node/project status
actant studio             Open local Studio dashboard
actant doctor             Diagnose setup/config/workers
actant examples           Browse and run examples
actant templates          List bundled templates
actant version
actant chat               Quick interactive chat session

# Generators
actant generate command   <name>
actant generate effect    <name>
actant generate worker    <kind>
actant generate agent     <name>
actant generate workflow  <name>

# Schema
actant schema validate
actant schema apply
actant schema diff
actant schema migrate
actant schema show

# Runtime surfaces
actant command run        <type> --input <json|@file>
actant effect list        [--type ...] [--status ...]
actant effect inspect     <id>
actant effect retry       <id>
actant worker list
actant worker start       <name>
actant worker logs        <name>
actant worker register    <kind>
actant agent list
actant agent run          <name>
actant session create     --agent <name>
actant session send       --to <session> "<text>"
actant session show       <id>
actant memory candidates
actant memory approve     <id>
actant memory reject      <id>
actant memory edit        <id>
actant memory list
actant memory show        <id>
actant memory trace       <id>
actant memory restrict    <id> [--local-only|--never-cloud|--never-sync]
actant memory delete      <id>
actant approval list      [--status pending|all]
actant approval show      <id>
actant approval approve   <id> [--once|--session|--scope|--forever]
actant approval deny      <id>
actant workflow list
actant workflow create    <name>
actant workflow run       <name>
actant workflow watch     <run_id>
actant workflow show      <id>
actant replay list
actant replay create      --from-event <id>
actant replay run         <id> [--model ...] [--tools read-only] [--without-memory <id>] [--policy <id>]
actant replay diff        <a> <b>
actant eval list
actant eval run           [--suite ...]
actant policy list
actant policy show        <id>
actant policy grant       <actor> <permission> [--resource <pat>] [--ceiling <sens>] [--approval ...] [--expires <iso>]
actant policy revoke      <scope_id>
actant policy test        --actor ... --effect ... --input ...
actant context list       [--session ...]
actant context show       <id>
actant context inspect    <id> [--open]
actant tool list
actant tool register
actant tool show          <name>
actant mcp add            <name>
actant mcp import         <file|url>
actant mcp list
actant mcp tools          <server>
actant mcp wrap           <tool> [--approval ...]

# SDK + deploy
actant sdk generate       --lang python|typescript|swift|rust
actant deploy local
actant deploy scaffold    docker-compose|k8s
actant deploy docker-compose up|down|logs
actant deploy cloud
actant deploy status
actant deploy logs

# Logs
actant logs               [--live] [--session ...] [--type ...]
```

## Project layout (scaffolded)

```
my-agent/
├── actant.yaml
├── README.md
├── .actant/
│   ├── local/                       # SQLite DB + artifacts in dev
│   └── cache/
├── schema/
│   ├── actors.actant
│   ├── sessions.actant
│   ├── memory.actant
│   ├── tools.actant
│   ├── workflows.actant
│   └── policies.actant
├── commands/                        # project commands (custom)
├── workers/                         # per-language worker stubs
├── agents/                          # agent definitions
├── workflows/                       # workflow .actant files
├── evals/                           # eval cases
├── examples/                        # runnable scripts (one per concept)
└── tests/
```

## The project config (`actant.yaml`)

```yaml
project:
  name: my-agent
  version: 0.1.0

node:
  mode: local
  port: 4739
  studio_port: 4740

storage:
  provider: sqlite
  path: .actant/local/actant.db

artifacts:
  path: .actant/artifacts

secrets:
  provider: env

workers:
  model:  { command: python workers/model_worker.py }
  shell:  { command: python workers/shell_worker.py, sandbox: docker }
  file:   { command: python workers/file_worker.py }

policies:
  default:
    cloud_model_visibility: low_only
    shell_requires_approval: true
    memory_requires_review: true

studio:
  enabled: true
```

`actant.yaml` is the project config. The `.actant` schema files are the **content** the CLI compiles into migrations + SDK types via `actant-schema-dsl`.

## Universal flags

Every command supports:

```
--json            Machine-readable output
--quiet           Minimal output
--yes             Skip confirmations
--dry-run         Plan only; do not mutate
--config <path>   Override actant.yaml location
--workspace <id>  Target a specific workspace
```

CI flow:

```
actant schema validate
actant policy validate
actant eval run --suite regression
actant replay run --suite critical
```

## CLI personality

- Clear. Fast. Structured.
- Non-magical (no surprise mutations).
- Explanatory when needed; quiet when automated.
- Beautiful but not cute.
- Every output ends with "the next useful command."

**Bad:** `Done!`

**Good:**

```
Created memory candidate memcand_42

Status:
  pending_review

Evidence:
  shell effect eff_12  (command: pytest -q)

Review:
  actant memory approve memcand_42
  actant memory reject memcand_42
```

## Diagnostics: `actant doctor`

```
actant doctor
```

```
Actant Doctor

Project
  ✓ actant.yaml valid
  ✓ schema valid
  ✓ SDK generated

Node
  ✓ local node running
  ✓ database reachable
  ✓ subscriptions working

Workers
  ✓ model_worker healthy
  ✓ shell_worker healthy
  ✗ browser_worker missing
    Fix: actant worker start browser

Policies
  ✓ default policy valid
  ⚠ shell.run requires approval but no approval UI active
    Fix: actant studio approvals

Secrets
  ✓ OPENAI_API_KEY configured
  ○ ANTHROPIC_API_KEY not configured

Replay
  ✓ checkpoints enabled
```

`actant doctor --fix` applies fixes whose action is `Fix: <command>`.

## Deployment scaffolding

```
actant deploy local                   # Phase 1
actant deploy scaffold docker-compose # Phase 1
actant deploy scaffold k8s            # Phase 4+
actant deploy docker-compose up       # Phase 2+
actant deploy cloud                   # Phase 6
actant deploy status
actant deploy logs
```

## Studio integration from the CLI

```
actant studio
actant studio approvals
actant studio session sess_123
actant studio replay replay_456
actant studio context ctx_789
actant studio memory
actant studio workflow run_111

actant context inspect ctx_789 --open
```

Each opens the exact dashboard view from `/planning/studio-design.md`.

## CLI v0.1 minimum (Phase 1)

This is the *must-ship* surface for the alpha demo. Everything else lives in later phases.

```
actant new                  --template minimal|coding-agent
actant init
actant dev
actant studio
actant status
actant doctor

actant schema validate
actant schema apply

actant memory candidates
actant memory approve
actant memory reject
actant memory show
actant memory trace

actant approval list
actant approval show
actant approval approve
actant approval deny

actant session create
actant session send
actant session show

actant replay create
actant replay run

actant examples run coding-agent
actant examples list

actant logs --live
```

All other subcommands ship in subsequent phases per `/planning/cli-by-phase.md` below.

## CLI staging across phases

| Phase | CLI additions                                                                       |
| ----- | ----------------------------------------------------------------------------------- |
| 1     | v0.1 minimum (above), `coding-agent` + `minimal` templates, three examples. Plus AI-native + reliability v0.1: `actant index init|add|build|search|inspect`, `actant retrieval inspect`, `actant queue list`, `actant dlq list`, `actant throttle status`, `actant budget status`, `actant observe enable`. |
| 2     | `effect`, `worker`, `tool`, `intent`, `intervene`, `mcp`, MCP wrap. Templates: `browser-agent`, `mcp-agent`, `desktop-agent`. Examples: `tool-approval`, `context-firewall`. New AI-native: `actant index reembed`, `actant mcp resources`, `actant trace export`, `actant throttle set`, `actant circuit list/open/reset`, `actant lock list`, `actant ingress list/test/replay`. |
| 3     | `memory restrict/expire/revoke/delete`, `context inspect`, capsule commands, trust commands. Templates: `support-agent`, `research-agent`. Examples: `memory-review` (full lifecycle). New: `actant index migrate` (embedding-space), `actant prompt create/render/eval/diff`, `actant model list/show/route`, `actant cache stats/clear/inspect`. |
| 4     | `workflow`, `eval`, `policy`, `generate workflow`, `agent` with workflows. Template: `multi-agent-board`. Examples: `workflow-dag`, `replay-debugging`. New: `actant retry policies/show/force/cancel`, `actant dlq retry/discard/convert-to-eval`, `actant a2a peers/discover/delegate`, `actant ap2 mandate grant/list/revoke`. |
| 5     | `replay diff`, `replay modes`, `replay suite`. Examples: `replay-debugging` (deeper). New: `actant replay run --embedder|--reranker|--without-memory|--retrieval-mode`. |
| 6     | `deploy cloud`, `cluster`, `policy` cross-workspace, `audit export`. Templates: `enterprise-agent`. |

## Engineering ownership

| Crate                       | Purpose                                                           |
| --------------------------- | ----------------------------------------------------------------- |
| `actant-cli`                | The binary + subcommand dispatch.                                 |
| `actant-templates`          | Bundled project templates + render engine.                        |
| `actant-schema-dsl`         | `.actant` DSL parser + compilers (→ SQL, → types, → stubs).        |
| `actant-codegen-project`    | `actant generate ...` family.                                     |
| `actant-sdk-codegen`        | (separate) SDK type emission from server metadata.                |

The CLI is one crate but it composes the four supporting crates above.

## What this is *not*

- Not a full TUI. `actant chat` is interactive but minimal; rich exploration lives in Studio.
- Not a clone of `temporal` or `kubectl`. We borrow the polish, not the verb hierarchy.
- Not a substitute for the SDKs. Anything you can do via the CLI you can do via an SDK; the CLI is for humans and CI, not application code.

## References

- ADR-0008: CLI as first-class product surface.
- ADR-0009: `.actant` schema DSL.
- `/planning/cli-templates.md`: template catalog.
- `/planning/cli-examples.md`: example catalog.
- `/agents/actant-cli.md`: implementation work package.
- `/agents/actant-templates.md`, `/agents/actant-schema-dsl.md`, `/agents/actant-codegen-project.md`.
