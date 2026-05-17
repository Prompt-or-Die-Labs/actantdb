# actant-templates

Bundled project templates for `actant new`. Each template lives under `/templates/<name>/` in the repo; this crate embeds them at build time (via `include_dir!`) so the CLI is a single binary.

Templates:

| Name                 | What it scaffolds                                                |
| -------------------- | ---------------------------------------------------------------- |
| `minimal`            | Smallest ActantDB project. Local node + one no-op command.       |
| `coding-agent`       | Agent with shell + file tools, approvals, memory, replay, evals. |
| `browser-agent`      | Browser worker, page observations, approval gates, replay.       |
| `support-agent`      | Customer-support workflow with ticket memory, escalation.        |
| `research-agent`     | Web/search workflow, source artifacts, citation memory.          |
| `desktop-agent`      | Local-first app agent with file permissions and local memory.    |
| `multi-agent-board`  | Task board, subagents, workflow DAGs, worker heartbeats.         |
| `mcp-agent`          | MCP tool import, approval gateway, tool-call tracing.            |
| `swift-mac-agent`    | Swift SDK + local node + app/daemon integration.                 |
| `enterprise-agent`   | Multi-user policies, audit export, team approvals (Phase 6+).    |

Owns:

- `Template` enum + `render(name, vars)` function.
- Variable substitution (project name, language choice, port numbers).
- Post-render hooks (initialize git, run `actant schema apply`, install SDK).
- Versioned: each template has its own version; `actant new` records the version in the scaffolded project for upgrade tooling.

See `agents/actant-templates.md` and `planning/cli-templates.md`.
