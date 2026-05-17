# actant-cli

The `actantdb` CLI.

Subcommands (Phase 1):

```
actantdb start                  # boot the local server
actantdb status                 # show server health
actantdb migrate                # apply migrations
actantdb workspace create <name>
actantdb actor list
actantdb actor grant <actor_id> <permission> --resource <pattern> --ceiling <sensitivity>
actantdb event tail [--session ...] [--type ...]
actantdb command <type> --input <json>     # power-user escape hatch
actantdb studio                 # open Studio (Phase 1: print URL)
```

Built from `src/main.rs`. The `actant_cli` library surface (`src/lib.rs`) exists so integration tests can drive subcommands without forking a process.

See `agents/actant-cli.md` for the work package.
