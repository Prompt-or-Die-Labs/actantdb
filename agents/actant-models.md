# Work package: `actant-models`

## Context

Model registry + routing. Per-model capability metadata (context window, tool support, JSON reliability, modalities, cost, latency, privacy class, locality). The router selects a model from `(context sensitivity, budget, latency goal, required capabilities)` and records the selection as a `model_route_decision` row.

## Specs to read first

- `/specs/14-extended-primitives.md` §12 (model_route_decision).
- `/specs/15-actant-index.md` §13 (model-specific formatting), §15 (retrieval-aware routing).
- `/specs/05-security-model.md` §4 (visibility — local-only vs cloud routes).

## Scope

```rust
pub struct ModelRegistry { /* loaded from model_registry table */ }

pub struct RouteRequest<'a> {
    pub workspace_id: &'a WorkspaceId,
    pub purpose: &'a str,              // 'planner'|'executor'|'critic'|'embedder'|...
    pub sensitivity_ceiling: Sensitivity,
    pub required_capabilities: &'a [Capability],
    pub budget_remaining_usd: Option<f64>,
    pub latency_goal_ms: Option<u32>,
    pub locality: Locality,            // Local | Cloud | EitherPreferLocal
}

pub struct Route { pub route_id: ModelRouteId, pub fallbacks: Vec<ModelRouteId>, pub reason: String }

impl ModelRegistry {
    pub async fn select(&self, tx: &mut Transaction<'_>, req: RouteRequest<'_>) -> Result<Route, RouteError>;
    pub fn show(&self, route: &ModelRouteId) -> Option<ModelEntry>;
}
```

### Internal modules

```
crates/actant-models/src/
├── lib.rs
├── registry.rs
├── select.rs                    (scoring + fallback ranking)
├── locality.rs                  (cloud / local routing rules)
└── error.rs
```

### Tests

- A `local_only` route is always chosen when sensitivity exceeds the cloud ceiling.
- A high-context-window requirement filters out small models.
- Every `select` produces a `model_route_decision` row with non-empty `selection_reason`.
- Fallbacks include at least one local model if any local model satisfies required capabilities.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] Selection is deterministic for the same inputs (modulo randomized tie-break which is seeded).

## Do NOT

- Do NOT route based on trust alone. Trust modulates risk; routing reads capability + sensitivity.
- Do NOT call `actant-policy` Guard from here; routing is a *suggestion*, Guard remains the final say.

## Hand-off

`just ci`.
