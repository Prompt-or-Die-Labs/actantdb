# actant-worker-model

Reference model-call worker for Phase 2.

Owns:

- HTTP adapter for OpenAI-compatible endpoints (OpenAI, Anthropic Messages-on-OpenAI-compat shim, vLLM, Ollama, MLX local OpenAI server, LM Studio).
- Per-provider auth from `secret_ref` (server fetches material into env on lease).
- Streaming tokens → progress observations.
- Token / cost accounting back into `model_call`.
- Honors the lease's visibility (`local_only` leases refuse to target a route that requires `cloud_model_allowed`).

Binary: `actant-worker-model`.

See `agents/actant-worker-model.md`.
