# actant-models

Model registry + routing. Per-model capability metadata (context window, tool support, JSON reliability, vision/audio/embedding/rerank support, cost, latency, privacy class, local/cloud). Routing selects the right model from context sensitivity, budget, latency goal, and required capabilities. Records every selection as a `model_route_decision` row.

See `agents/actant-models.md`.
