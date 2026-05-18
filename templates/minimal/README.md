# {{project_name}}

Minimal ActantDB project scaffolded from the `minimal` template.

This project wires `@actantdb/core` (embedded ledger on `node:sqlite`) into a tiny
no-op agent through `withActant({...})`. There is no real model call — just enough
plumbing so you can verify your install, open Studio, and start replacing the stub.

## Layout

```
{{project_name}}/
  package.json
  README.md
  index.mjs            # entry point — withActant wraps a no-op agent
  .env.example         # copy to .env if you need env vars
```

## Run it

```bash
pnpm install
node ./index.mjs                  # records one run into ./.actantdb
pnpm studio                       # opens Studio on http://127.0.0.1:{{studio_port}}
```

The default substrate port is `{{port}}` (for the optional `actantdb-server`); the
embedded path used here does not need it.

## Next steps

- Swap the no-op `generate` for a real model call.
- Add tools to the `tools` record (the `coding-agent` template shows the shape).
- Define a real policy in `@actantdb/policy` and pass it via `withActant({ policy })`.
