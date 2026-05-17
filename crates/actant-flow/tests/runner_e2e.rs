//! End-to-end: walk a daily-digest workflow through the Runner with real
//! handlers wired in. No actual external I/O — handlers just compute
//! deterministic responses so the test is fast + portable.

use actant_flow::{Action, Edge, Node, NodeType, Runner, StepStatus, Workflow};
use serde_json::json;

fn daily_digest() -> Workflow {
    Workflow {
        name: "daily-digest".into(),
        nodes: vec![
            Node {
                key: "fetch_inbox".into(),
                node_type: NodeType::ToolCall,
                config: Some(json!({"tool": "gmail.list_unread", "arguments": {}})),
            },
            Node {
                key: "fetch_calendar".into(),
                node_type: NodeType::ToolCall,
                config: Some(json!({"tool": "calendar.read", "arguments": {}})),
            },
            Node {
                key: "summarize".into(),
                node_type: NodeType::ModelCall,
                config: Some(json!({"prompt": "summarize {{inbox}} {{calendar}}"})),
            },
            Node {
                key: "approve".into(),
                node_type: NodeType::ApprovalGate,
                config: Some(json!({"summary": "Send digest?"})),
            },
            Node {
                key: "send".into(),
                node_type: NodeType::ToolCall,
                config: Some(json!({"tool": "message.send", "arguments": {"to":"wes"}})),
            },
        ],
        edges: vec![
            Edge {
                from: "fetch_inbox".into(),
                to: "summarize".into(),
            },
            Edge {
                from: "fetch_calendar".into(),
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

#[tokio::test]
async fn daily_digest_walks_to_completion() {
    let wf = daily_digest();
    let mut r = Runner::new(wf).unwrap();
    let mut tool_invocations = Vec::new();
    let mut model_invocations = Vec::new();
    let mut approval_resolved = false;

    loop {
        match r.next_action() {
            Action::ToolCall {
                node_key,
                tool,
                args,
            } => {
                tool_invocations.push((tool.clone(), args));
                r.complete_step(&node_key, json!({"ok": true, "tool": tool}));
            }
            Action::ModelCall { node_key, prompt } => {
                model_invocations.push(prompt);
                r.complete_step(&node_key, json!({"summary": "5 emails, 3 events"}));
            }
            Action::AwaitApproval {
                node_key,
                summary: _,
            } => {
                approval_resolved = true;
                r.resolve_approval(&node_key, true);
            }
            Action::Delay { node_key, .. } => {
                r.complete_step(&node_key, json!({}));
            }
            Action::Done { run } => {
                assert_eq!(run.status, StepStatus::Succeeded);
                break;
            }
        }
    }

    // 2 tool calls (fetch_inbox + fetch_calendar) + 1 send = 3 total.
    assert_eq!(tool_invocations.len(), 3);
    assert!(tool_invocations
        .iter()
        .any(|(t, _)| t == "gmail.list_unread"));
    assert!(tool_invocations.iter().any(|(t, _)| t == "calendar.read"));
    assert!(tool_invocations.iter().any(|(t, _)| t == "message.send"));
    assert_eq!(model_invocations.len(), 1);
    assert!(approval_resolved);
}

#[tokio::test]
async fn daily_digest_with_denial_stops_at_gate() {
    let mut r = Runner::new(daily_digest()).unwrap();
    loop {
        match r.next_action() {
            Action::ToolCall { node_key, .. } => {
                r.complete_step(&node_key, json!({"ok": true}));
            }
            Action::ModelCall { node_key, .. } => {
                r.complete_step(&node_key, json!({}));
            }
            Action::AwaitApproval { node_key, .. } => {
                r.resolve_approval(&node_key, false);
            }
            Action::Delay { node_key, .. } => {
                r.complete_step(&node_key, json!({}));
            }
            Action::Done { run } => {
                assert_eq!(run.status, StepStatus::Failed);
                let send_step = run.steps.get("send").unwrap();
                assert_eq!(send_step.status, StepStatus::Pending);
                return;
            }
        }
    }
}
