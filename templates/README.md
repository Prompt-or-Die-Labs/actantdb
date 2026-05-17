# templates/

Project templates for `actant new`. Each subdirectory is a self-contained scaffold that `actant-templates` embeds in the CLI binary at build time.

See `/planning/cli-templates.md` for the canonical catalog and per-template feature list.

## Ship order

| Phase | Templates                                                          |
| ----- | ------------------------------------------------------------------ |
| 1     | `minimal`, `coding-agent`                                          |
| 2     | `browser-agent`, `mcp-agent`, `desktop-agent`                      |
| 3     | `support-agent`, `research-agent`                                  |
| 4     | `multi-agent-board`                                                |
| 6     | `swift-mac-agent` (or earlier if Swoosh demands), `enterprise-agent`|

## Convention

Every template directory contains:

```
templates/<name>/
├── template.yaml              (metadata: version, languages, features, phase)
├── README.md                  (what this template gives you + next commands)
└── <project tree>             (the files scaffolded into the user's project)
```

The CLI's render step substitutes variables (project name, ports, language choice) and runs post-render hooks (`git init`, `actant schema apply`, SDK install).

## Tests

Each template has an integration test (`crates/actant-cli/tests/templates_<name>.rs`) that:

1. Scaffolds into a temp directory.
2. Runs `actant dev --headless --timeout 60s`.
3. Asserts the doctor passes.
4. Asserts the example script in the template succeeds.

See `/agents/actant-templates.md` for the work package.
