# ActantDB recipes

Ten focused, copy-pasteable how-tos. Each one solves one common task with
runnable code that compiles against the published `@actantdb/*` packages.

| #  | Recipe                                                                     |
|----|----------------------------------------------------------------------------|
| 01 | [Add an approval gate to a tool](./01-add-approval-to-a-tool.md)           |
| 02 | [Replay last night's failed run](./02-replay-last-nights-failed-run.md)    |
| 03 | [Wire ActantDB into Next.js](./03-wire-into-nextjs.md)                     |
| 04 | [Use Ollama only — no cloud models](./04-use-ollama-only-no-cloud-models.md) |
| 05 | [Test an agent with snapshots](./05-test-an-agent-with-snapshots.md)       |
| 06 | [Export to BigQuery](./06-export-to-bigquery.md)                           |
| 07 | [Share a replay session](./07-share-a-replay-session.md)                   |
| 08 | [Audit-export to S3 on a schedule](./08-audit-export-to-s3-on-a-schedule.md) |
| 09 | [Add ActantDB to an existing Mastra app](./09-add-actantdb-to-an-existing-mastra-app.md) |
| 10 | [Build your first MCP tool on top of ActantDB](./10-build-your-first-mcp-tool-on-top-of-actantdb.md) |

## Conventions

- Every recipe targets `@actantdb/*@0.0.13` or later.
- Snippets default to ESM (`.mjs` or `.ts`). Node ≥22.5 required.
- Where a snippet wraps an agent, the wrapper is framework-agnostic — Mastra,
  LangGraph, OpenAI Agents SDK, hand-rolled. Pick what you have.
- Recipes that hit external services (BigQuery, S3, Ollama) keep credentials
  in env vars; no recipe writes secrets to disk.
