# {{project_name}}

Coding agent scaffolded from the `coding-agent` template. Mirrors the alpha demo
under `/examples/test-cleanup`.

A Mastra-shaped agent with `shell.run` and `file.write` tools, wrapped through
`@actantdb/mastra` so Guard intercepts risky proposals and Chronicle records
the full causal trace.

## Layout

```
{{project_name}}/
  package.json
  README.md
  agent.mjs            # stubbed planner + shell/file tools, wrapped via withActant
  .env.example         # copy to .env if you need env vars
```

## Run it

```bash
pnpm install
node ./agent.mjs                  # records one constrained run
pnpm studio                       # opens Studio on http://127.0.0.1:{{studio_port}}
```

In Studio: click the `model_call` row, hit **Replay from here**, toggle
"stricter policy" or "exclude mem_42_dist", and re-run to see Guard rewrite
the proposed shell command.

## What this proves

- **Guard Authority** rewrites the destructive command before the shell runs.
- **Chronicle Replay** reruns the planner under a different memory + policy.
- **Context manifests** make the model's input inspectable.

## Next steps

- Replace the stub `generate` with a real model call (Anthropic / OpenAI / local).
- Add more tools to `agent.tools` — each gets policy-checked at call time.
- Define a stricter policy in `@actantdb/policy` and pass it via `withActant({ policy })`.

The default substrate port is `{{port}}`; Studio binds to `{{studio_port}}`.
