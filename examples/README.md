# examples/

Runnable examples for `actant examples run <name>`. Each scaffolds from a template, runs a scripted interaction, and prints next steps. Designed to make ActantDB observable end-to-end in under five minutes.

See `/planning/cli-examples.md` for the canonical catalog and per-example flow.

## Ship order

| Phase | Examples                                              |
| ----- | ----------------------------------------------------- |
| 1     | `coding-agent`                                        |
| 2     | `tool-approval`, `mcp-github`                         |
| 3     | `memory-review`, `context-firewall`, `swoosh-scout`   |
| 4     | `workflow-dag`                                        |
| 5     | `replay-debugging`                                    |

## Convention

Each example directory:

```
examples/<name>/
├── README.md                  (what this teaches + commands shown)
├── example.yaml               (manifest: template + script + duration)
├── run.{py|ts|swift|rs}       (the scripted interaction)
└── expected_output.txt        (CI snapshot)
```

The CLI's `actant examples run <name>`:

1. Scaffolds a temp project from `example.yaml`'s `template:`.
2. Boots `actant dev` in the background.
3. Executes `run.{ext}`.
4. Compares stdout to `expected_output.txt` if `--snapshot` is set.
5. Prints the next useful CLI commands.

## Tests

CI runs every example as `actant examples run <name> --headless`, asserts exit code 0, and snapshots output. Failures here block merge.
