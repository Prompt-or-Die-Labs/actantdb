# Work package: `actant-flow`

## Context

`actant-flow` is the Flow Engine. Phase 1 ships **types and traits only** — the executor itself lands in Phase 4. The reason: other crates (especially `actant-command` and `actant-server`) need to reference the workflow types even before the executor exists.

## Specs to read first

- `/specs/07-workflows-and-replay.md` §§1–5.
- `/specs/02-data-model.sql` — `workflow`, `workflow_node`, `workflow_edge`, `workflow_run`, `workflow_step_run`, `trigger`, `agent_task`.

## Scope (Phase 1)

### Public API surface

```rust
pub struct WorkflowDefinition {
    pub id: WorkflowId,
    pub name: String,
    pub version: u32,
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
}

pub struct WorkflowNode {
    pub key: String,
    pub node_type: NodeType,
    pub config: serde_json::Value,
    pub required_permissions: Vec<String>,
    pub retry_policy: Option<RetryPolicy>,
    pub timeout_policy: Option<TimeoutPolicy>,
}

pub enum NodeType {
    AgentTask, ModelCall, ToolCall, ApprovalGate, HumanTask, Condition,
    ParallelGroup, MemoryWrite, FileOperation, BrowserAction, ExternalWebhook,
    Delay, Subworkflow,
}

pub struct WorkflowEdge {
    pub from: String,           // node_key
    pub to: String,             // node_key
    pub condition: Option<String>,
    pub order_index: i32,
}

pub trait WorkflowExecutor: Send + Sync {
    async fn start(&self, def: WorkflowDefinition, input: serde_json::Value)
        -> Result<WorkflowRunId, FlowError>;
    // ... advance, cancel, etc. — full impl in Phase 4
}
```

### Internal modules

```
crates/actant-flow/src/
├── lib.rs
├── definition.rs              // types
├── node_type.rs
├── executor.rs                // trait
├── retry.rs                   // RetryPolicy
├── timeout.rs                 // TimeoutPolicy
└── error.rs
```

### Tests

- Type round-trip: every `NodeType` serializes/deserializes through serde matching the snake_case form in specs.
- `WorkflowDefinition` round-trip via JSON.

## Acceptance criteria

- [ ] `cargo build -p actant-flow` zero warnings.
- [ ] `cargo test -p actant-flow` passes.
- [ ] `cargo clippy -p actant-flow -- -D warnings` passes.
- [ ] Every node type listed in `/specs/07-workflows-and-replay.md` §2 has a variant.

## Do NOT

- Do NOT implement the executor in Phase 1. Stub the trait.
- Do NOT add cron, webhook, or event-trigger runtime. Phase 4.
- Do NOT use `unsafe`.

## Hand-off

`just ci`.
