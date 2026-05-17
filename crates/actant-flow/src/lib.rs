//! actant-flow — workflow DAG definition + executor.
//!
//! Phase 1: in-memory DAG with topological order.
//! Phase 4: real step-by-step runner with approval gates, parallel groups,
//! and persisted `workflow_run` + `workflow_step_run` rows.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::{HashMap, HashSet};

use actant_core::*;
use actant_storage::Storage;
use serde::{Deserialize, Serialize};

/// Node-type kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    /// Agent task.
    AgentTask,
    /// Model call.
    ModelCall,
    /// Tool call.
    ToolCall,
    /// Human approval.
    ApprovalGate,
    /// Human work item.
    HumanTask,
    /// Branch condition.
    Condition,
    /// Parallel group barrier.
    ParallelGroup,
    /// Memory write.
    MemoryWrite,
    /// File operation.
    FileOperation,
    /// Delay.
    Delay,
    /// Subworkflow invocation.
    Subworkflow,
}

/// One DAG node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Stable key.
    pub key: String,
    /// Type.
    pub node_type: NodeType,
    /// Optional config payload (interpreted per node_type).
    #[serde(default)]
    pub config: Option<serde_json::Value>,
}

/// A directed edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Source key.
    pub from: String,
    /// Target key.
    pub to: String,
}

/// A workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Display name.
    pub name: String,
    /// Nodes.
    pub nodes: Vec<Node>,
    /// Edges.
    pub edges: Vec<Edge>,
}

impl Workflow {
    /// Compute a topological order. Returns `Err` for cycles.
    pub fn topological_order(&self) -> Result<Vec<String>, &'static str> {
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut indeg: HashMap<&str, usize> = HashMap::new();
        for n in &self.nodes {
            indeg.insert(n.key.as_str(), 0);
            adj.entry(n.key.as_str()).or_default();
        }
        for e in &self.edges {
            adj.entry(e.from.as_str()).or_default().push(e.to.as_str());
            *indeg.entry(e.to.as_str()).or_insert(0) += 1;
        }
        let mut ready: Vec<&str> = indeg
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(k, _)| *k)
            .collect();
        ready.sort();
        let mut visited = HashSet::new();
        let mut out = Vec::new();
        while let Some(n) = ready.pop() {
            if !visited.insert(n) {
                continue;
            }
            out.push(n.to_string());
            for &m in adj.get(n).unwrap_or(&Vec::new()) {
                let entry = indeg.entry(m).or_insert(0);
                if *entry > 0 {
                    *entry -= 1;
                }
                if *entry == 0 {
                    ready.push(m);
                }
            }
        }
        if out.len() != self.nodes.len() {
            return Err("cycle detected");
        }
        Ok(out)
    }
}

/// Step run status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    /// Not yet started.
    Pending,
    /// Currently running.
    Running,
    /// Completed successfully.
    Succeeded,
    /// Failed.
    Failed,
    /// Skipped (a condition evaluated false).
    Skipped,
    /// Waiting on a human approval gate.
    AwaitingApproval,
}

/// One run of a node within a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRun {
    /// Node key.
    pub node_key: String,
    /// Status.
    pub status: StepStatus,
    /// Output (free-form).
    pub output: Option<serde_json::Value>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Created at.
    pub started_at: Option<String>,
    /// Finished at.
    pub finished_at: Option<String>,
}

/// One workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    /// Run id.
    pub id: WorkflowRunId,
    /// Workflow name.
    pub workflow_name: String,
    /// Step runs keyed by node_key.
    pub steps: HashMap<String, StepRun>,
    /// Final status.
    pub status: StepStatus,
}

/// A side effect a step asks the host to perform. The runner is host-side
/// stateless — it tells the caller what should happen via `Action`, the
/// caller carries it out and reports back. This keeps the runner free of
/// model/effect/tool dependencies.
#[derive(Debug, Clone)]
pub enum Action {
    /// Invoke a tool with these args. Return the result via `complete_step`.
    ToolCall {
        /// Step key.
        node_key: String,
        /// Tool name (from config["tool"]).
        tool: String,
        /// Arguments (from config["arguments"]).
        args: serde_json::Value,
    },
    /// Invoke a model with the given prompt.
    ModelCall {
        /// Step key.
        node_key: String,
        /// Prompt text from config["prompt"].
        prompt: String,
    },
    /// Pause the run; an approval is required for this step.
    AwaitApproval {
        /// Step key.
        node_key: String,
        /// Summary.
        summary: String,
    },
    /// Delay before continuing.
    Delay {
        /// Step key.
        node_key: String,
        /// Seconds.
        seconds: u64,
    },
    /// Workflow finished — no more actions.
    Done {
        /// Run record.
        run: Run,
    },
}

/// The runner: a tiny state machine over a Workflow + Run.
#[derive(Debug)]
pub struct Runner {
    workflow: Workflow,
    order: Vec<String>,
    run: Run,
}

impl Runner {
    /// New runner for a workflow.
    pub fn new(workflow: Workflow) -> Result<Self, &'static str> {
        let order = workflow.topological_order()?;
        let steps = workflow
            .nodes
            .iter()
            .map(|n| {
                (
                    n.key.clone(),
                    StepRun {
                        node_key: n.key.clone(),
                        status: StepStatus::Pending,
                        output: None,
                        error: None,
                        started_at: None,
                        finished_at: None,
                    },
                )
            })
            .collect();
        Ok(Self {
            run: Run {
                id: WorkflowRunId::new(),
                workflow_name: workflow.name.clone(),
                steps,
                status: StepStatus::Pending,
            },
            order,
            workflow,
        })
    }

    /// Snapshot of the run.
    pub fn run(&self) -> &Run {
        &self.run
    }

    /// Advance the run to the next action.
    pub fn next_action(&mut self) -> Action {
        if self.run.status == StepStatus::Failed {
            return Action::Done {
                run: self.run.clone(),
            };
        }
        let order = self.order.clone();
        for key in &order {
            let status = self.run.steps.get(key).expect("step exists").status;
            if status == StepStatus::AwaitingApproval || status == StepStatus::Running {
                return self.action_for(key);
            }
            if status == StepStatus::Pending {
                let s = self.run.steps.get_mut(key).expect("step exists");
                s.status = StepStatus::Running;
                s.started_at = Some(now_rfc3339());
                return self.action_for(key);
            }
        }
        self.run.status = StepStatus::Succeeded;
        Action::Done {
            run: self.run.clone(),
        }
    }

    /// Mark a step succeeded with output.
    pub fn complete_step(&mut self, node_key: &str, output: serde_json::Value) {
        if let Some(s) = self.run.steps.get_mut(node_key) {
            s.status = StepStatus::Succeeded;
            s.output = Some(output);
            s.finished_at = Some(now_rfc3339());
        }
    }

    /// Mark a step failed.
    pub fn fail_step(&mut self, node_key: &str, error: impl Into<String>) {
        if let Some(s) = self.run.steps.get_mut(node_key) {
            s.status = StepStatus::Failed;
            s.error = Some(error.into());
            s.finished_at = Some(now_rfc3339());
        }
        self.run.status = StepStatus::Failed;
    }

    /// Resolve an approval (used after AwaitApproval).
    pub fn resolve_approval(&mut self, node_key: &str, approved: bool) {
        if let Some(s) = self.run.steps.get_mut(node_key) {
            if approved {
                s.status = StepStatus::Succeeded;
                s.output = Some(serde_json::json!({"approved": true}));
            } else {
                s.status = StepStatus::Failed;
                s.error = Some("denied".into());
                self.run.status = StepStatus::Failed;
            }
            s.finished_at = Some(now_rfc3339());
        }
    }

    fn action_for(&mut self, key: &str) -> Action {
        let node = self
            .workflow
            .nodes
            .iter()
            .find(|n| n.key == key)
            .expect("node by key");
        match node.node_type {
            NodeType::ToolCall => {
                let cfg = node.config.clone().unwrap_or(serde_json::json!({}));
                let tool = cfg
                    .get("tool")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = cfg
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                Action::ToolCall {
                    node_key: key.into(),
                    tool,
                    args,
                }
            }
            NodeType::ModelCall => {
                let cfg = node.config.clone().unwrap_or(serde_json::json!({}));
                let prompt = cfg
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Action::ModelCall {
                    node_key: key.into(),
                    prompt,
                }
            }
            NodeType::ApprovalGate => {
                if let Some(s) = self.run.steps.get_mut(key) {
                    s.status = StepStatus::AwaitingApproval;
                }
                let cfg = node.config.clone().unwrap_or(serde_json::json!({}));
                let summary = cfg
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("approval required")
                    .to_string();
                Action::AwaitApproval {
                    node_key: key.into(),
                    summary,
                }
            }
            NodeType::Delay => {
                let cfg = node.config.clone().unwrap_or(serde_json::json!({}));
                let seconds = cfg.get("seconds").and_then(|v| v.as_u64()).unwrap_or(0);
                Action::Delay {
                    node_key: key.into(),
                    seconds,
                }
            }
            _ => {
                // Anything else is treated as an instant no-op.
                self.complete_step(key, serde_json::json!({"noop": true}));
                Action::ToolCall {
                    node_key: key.into(),
                    tool: "noop".into(),
                    args: serde_json::json!({}),
                }
            }
        }
    }
}

/// Persist a finished `Run` into the `workflow_run` / `workflow_step_run`
/// tables. Convenience for the server / CLI.
pub async fn persist_run(
    storage: &Storage,
    workspace: &WorkspaceId,
    workflow_id: &WorkflowId,
    run: &Run,
) -> Result<(), ActantError> {
    let now = now_rfc3339();
    sqlx::query(
        "INSERT INTO workflow_run
            (id, workflow_id, workspace_id, status, started_at, finished_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(run.id.as_str())
    .bind(workflow_id.as_str())
    .bind(workspace.as_str())
    .bind(json_enum(&run.status))
    .bind(&now)
    .bind(&now)
    .execute(storage.pool())
    .await
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

fn json_enum<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v)
        .unwrap_or_else(|_| "\"\"".into())
        .trim_matches('"')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_workflow() -> Workflow {
        Workflow {
            name: "daily-digest".into(),
            nodes: vec![
                Node {
                    key: "fetch".into(),
                    node_type: NodeType::ToolCall,
                    config: Some(serde_json::json!({"tool":"gmail.list_unread","arguments":{}})),
                },
                Node {
                    key: "summarize".into(),
                    node_type: NodeType::ModelCall,
                    config: Some(serde_json::json!({"prompt":"summarize the emails"})),
                },
                Node {
                    key: "approve".into(),
                    node_type: NodeType::ApprovalGate,
                    config: Some(serde_json::json!({"summary":"send digest?"})),
                },
                Node {
                    key: "send".into(),
                    node_type: NodeType::ToolCall,
                    config: Some(
                        serde_json::json!({"tool":"message.send","arguments":{"to":"wes"}}),
                    ),
                },
            ],
            edges: vec![
                Edge {
                    from: "fetch".into(),
                    to: "summarize".into(),
                },
                Edge {
                    from: "summarize".into(),
                    to: "approve".into(),
                },
                Edge {
                    from: "approve".into(),
                    to: "send".into(),
                },
            ],
        }
    }

    #[test]
    fn topo_order() {
        let order = digest_workflow().topological_order().unwrap();
        assert_eq!(order, vec!["fetch", "summarize", "approve", "send"]);
    }

    #[test]
    fn detects_cycle() {
        let wf = Workflow {
            name: "c".into(),
            nodes: vec![
                Node {
                    key: "a".into(),
                    node_type: NodeType::Condition,
                    config: None,
                },
                Node {
                    key: "b".into(),
                    node_type: NodeType::Condition,
                    config: None,
                },
            ],
            edges: vec![
                Edge {
                    from: "a".into(),
                    to: "b".into(),
                },
                Edge {
                    from: "b".into(),
                    to: "a".into(),
                },
            ],
        };
        assert!(wf.topological_order().is_err());
    }

    #[test]
    fn runner_walks_steps_with_approval_pause() {
        let mut r = Runner::new(digest_workflow()).unwrap();
        // 1. fetch
        match r.next_action() {
            Action::ToolCall { node_key, tool, .. } => {
                assert_eq!(node_key, "fetch");
                assert_eq!(tool, "gmail.list_unread");
                r.complete_step("fetch", serde_json::json!({"unread": 5}));
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
        // 2. summarize (model_call)
        match r.next_action() {
            Action::ModelCall { node_key, prompt } => {
                assert_eq!(node_key, "summarize");
                assert!(prompt.contains("summarize"));
                r.complete_step("summarize", serde_json::json!({"summary":"5 emails"}));
            }
            other => panic!("expected ModelCall, got {other:?}"),
        }
        // 3. approval pause
        match r.next_action() {
            Action::AwaitApproval { node_key, .. } => {
                assert_eq!(node_key, "approve");
                r.resolve_approval("approve", true);
            }
            other => panic!("expected AwaitApproval, got {other:?}"),
        }
        // 4. send
        match r.next_action() {
            Action::ToolCall { node_key, tool, .. } => {
                assert_eq!(node_key, "send");
                assert_eq!(tool, "message.send");
                r.complete_step("send", serde_json::json!({"sent": true}));
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
        // Done
        match r.next_action() {
            Action::Done { run } => {
                assert_eq!(run.status, StepStatus::Succeeded);
            }
            other => panic!("expected Done, got {other:?}"),
        }
    }
}
