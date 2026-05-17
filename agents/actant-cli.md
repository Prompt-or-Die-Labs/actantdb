# Work package: `actant-cli`

## Context

`actant-cli` is the **flagship developer surface** for ActantDB. The CLI is not a project-scaffolding sidecar — it is the primary interface for scaffolding, running, inspecting, replaying, and shipping governed autonomous agents. A developer should be able to go from `actant new` to a working governed agent in under five minutes, without manually wiring memory, tools, approvals, traces, or workflows.

Binary name: `actant`. Optional daemon: `actantd`. The crate also re-exposes a small library surface (`actant_cli`) so integration tests can drive subcommands without forking processes.

The full design lives in `/planning/cli-design.md`. This work package implements **CLI v0.1** (the Phase 1 minimum) with structure that makes Phase 2-6 additions trivial.

## Specs to read first

- `/planning/cli-design.md` — full command map, personality, staging.
- `/planning/cli-templates.md` — template catalog (Phase 1 ships `minimal` + `coding-agent`).
- `/planning/cli-examples.md` — example catalog (Phase 1 ships `coding-agent`).
- `/specs/10-alpha-demo.md` — the alpha demo the CLI must drive end-to-end.
- `/specs/11-roadmap.md` Phase 1 — decision-gate UX.
- `/specs/08-api-spec.md` — the API the CLI calls.
- `/specs/adr/0008-cli-first-class.md` — the philosophy and commitment.

## Scope (Phase 1 — v0.1)

### Subcommands (must ship)

```
# Lifecycle
actant new                     [--template <name>] [--language <py|ts|swift|rust>] [--with <feature,...>] [--no-studio]
actant init                    [--template <name>]
actant dev                     [--no-studio] [--port N]
actant start                   [--db PATH] [--port N] [--bind ADDR]
actant stop
actant status
actant studio                  [<subview>]
actant doctor                  [--fix] [--json]
actant version
actant logs                    [--live] [--session ID] [--type T]
actant chat                    [--agent NAME]

# Examples + templates
actant examples list
actant examples run            <name> [--headless]
actant templates list

# Schema
actant schema validate
actant schema apply
actant schema show

# Runtime
actant session create          --agent NAME
actant session send            --to SESSION "<text>"
actant session show            <id>
actant approval list           [--status pending|all]
actant approval show           <id>
actant approval approve        <id> [--once|--session]
actant approval deny           <id>
actant memory candidates
actant memory approve          <id>
actant memory reject           <id>
actant memory edit             <id>
actant memory show             <id>
actant memory trace            <id>
actant replay create           --from-event <id>
actant replay run              <id>

# Power user
actant command run             <type> --input <json|@file>
```

All other subcommands listed in `/planning/cli-design.md` ship in Phase 2-6 per the staging table in that doc.

### Universal flags

```
--json --quiet --yes --dry-run --config <path> --workspace <id>
```

Every output ends with the next useful command. Bad: `Done!` Good: a status block plus a "Next:" section.

### Internal modules

```
crates/actant-cli/src/
├── main.rs                       (clap derive entry; binary name = `actant`)
├── lib.rs                        (subcommand handlers, exposed for tests)
├── commands/
│   ├── mod.rs
│   ├── new.rs                    (template render + post-render hooks)
│   ├── init.rs
│   ├── dev.rs                    (parallel boot: node + workers + Studio)
│   ├── start.rs
│   ├── stop.rs
│   ├── status.rs
│   ├── studio.rs
│   ├── doctor.rs
│   ├── version.rs
│   ├── logs.rs
│   ├── chat.rs                   (line-based REPL against a session)
│   ├── examples.rs
│   ├── templates.rs
│   ├── schema.rs
│   ├── session.rs
│   ├── approval.rs
│   ├── memory.rs
│   ├── replay.rs
│   └── command.rs                (the `actant command run ...` escape hatch)
├── config.rs                     (read/write actant.yaml)
├── output.rs                     (text + JSON renderers; "Next:" hint formatter)
├── client.rs                     (uses sdks/rust client where appropriate; for the embedded launch path may also call in-proc)
└── boot.rs                       (compose actant-server + workers + Studio for `dev`)
```

### Project config (`actant.yaml`)

The CLI reads `actant.yaml` (see `/planning/cli-design.md` §"The project config"). It must:

- Validate against a schema (recorded in `actant-cli/src/config.rs`).
- Refuse unknown top-level keys (forward-compat: warn with a known-keys list).
- Resolve `${env:VAR}` interpolation only inside `secrets:` and `workers:` blocks.

### Tests

- `actant new my --template minimal` scaffolds, `cargo run -p actant-cli -- dev --no-studio --headless` boots in under 30s.
- `actant doctor` on a fresh scaffold returns clean.
- `actant approval approve <id> --once` round-trips against an in-process server.
- `actant memory trace <id>` prints the full provenance chain.
- `--json` outputs are stable and parse with a JSON schema.
- Snapshot tests under `tests/snapshots/` for the help text of every subcommand.

## Acceptance criteria

- [ ] `cargo build -p actant-cli` zero warnings.
- [ ] `cargo test -p actant-cli` passes.
- [ ] `cargo clippy -p actant-cli -- -D warnings` passes.
- [ ] **The 5-minute test:** on a fresh machine, `cargo install --path crates/actant-cli && actant new my-agent --template coding-agent && cd my-agent && actant dev` produces a working approval flow within 5 minutes total wall-clock time.
- [ ] **The `actant examples run coding-agent` test:** completes end-to-end (creates a temp project, boots, runs the scripted interaction, prints next-step commands).
- [ ] Every CLI v0.1 command above has at least one test in `tests/`.
- [ ] `actant doctor --json` output validates against a documented schema.

## Do NOT

- Do NOT prompt for input outside `new` and `chat`. Other commands are scriptable.
- Do NOT print secrets. Token-bearing output is gated behind `--show-token` and prints only first/last 4 chars by default.
- Do NOT add destructive subcommands without `--yes` (or interactive confirmation that respects `--yes`).
- Do NOT make `actant dev` magical. Print every step; the developer should always know what's running.
- Do NOT shell out to `cargo` or `git` from inside `dev`. Boot composition is in-process.
- Do NOT use `unsafe`.

## Hand-off

`just ci`. Then run the 5-minute test from a clean home directory and time it.
