# actant-demo-ollama-only

Local-model-only demo: one Ollama generation is recorded as `model_call`, then a cloud completion tool is attempted and blocked by Guard before it executes.

## Run it

```bash
pnpm install
ollama pull llama3.2:8b
node demo.mjs
npx actantdb studio --project demo-ollama-only --store-dir ./.actantdb
```

For deterministic CI or a laptop without Ollama running:

```bash
ACTANTDB_OLLAMA_MOCK=1 node demo.mjs
```

## What you see

Studio shows:

- `model_call` with `model: "ollama:llama3.2:8b"`
- `tool_call_requested` for `openai_completion`
- `guard_verdict` with `decision: "block"`
- `tool_call_completed` with `status: "blocked"`

## Product boundary

ActantDB does not call the model. Your app calls Ollama directly; ActantDB records the model call metadata and gates tools through the same wrapper used by the Mastra, LangGraph, and CLI examples.
