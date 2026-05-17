# actant-circuit

Circuit breakers + provider health tracking. States: closed | open | half_open | degraded. Per (dependency_key) — provider, tool, worker, MCP server, A2A peer. Adaptive thresholds. Drives `actant-models` fallback routing. Emits `circuit_state_changed` events into the chronicle.

See `agents/actant-circuit.md`.
