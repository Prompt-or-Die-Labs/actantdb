//! actant-trigger — registry + scheduler for workflow triggers.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex};

/// A trigger kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Trigger {
    /// Cron schedule.
    Cron {
        /// Cron expression.
        expression: String,
    },
    /// Event-bus message.
    Event {
        /// Event source.
        source: String,
        /// Event type.
        event_type: String,
    },
    /// HTTP webhook.
    Webhook {
        /// Path.
        path: String,
    },
    /// Manual dispatch.
    Manual,
}

impl Trigger {
    /// Parse + validate the trigger.
    pub fn validate(&self) -> bool {
        match self {
            Trigger::Cron { expression } => cron::Schedule::from_str(expression).is_ok(),
            _ => true,
        }
    }
}

/// Trigger registration row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registration {
    /// Unique id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Trigger spec.
    pub trigger: Trigger,
    /// Workflow to fire.
    pub workflow_name: String,
    /// Last-fire timestamp (RFC3339).
    pub last_fired_at: Option<String>,
    /// Enabled.
    pub enabled: bool,
}

/// What the scheduler tells the host to do.
#[derive(Debug, Clone)]
pub enum FireAction {
    /// Fire this workflow.
    Fire {
        /// Registration id.
        trigger_id: String,
        /// Workflow name.
        workflow_name: String,
    },
}

/// In-memory scheduler. Wraps a `Mutex<HashMap<id, Registration>>` and a
/// `tokio::sync::watch` shutdown channel. The scheduler is intentionally
/// passive: `tick(now_secs)` evaluates which cron triggers should fire at
/// the given time and returns the list. The host owns the timer loop.
#[derive(Debug, Clone)]
pub struct Scheduler {
    inner: Arc<SchedulerInner>,
}

#[derive(Debug)]
struct SchedulerInner {
    regs: Mutex<HashMap<String, Registration>>,
}

impl Scheduler {
    /// New empty scheduler.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(SchedulerInner {
                regs: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Register a trigger.
    pub async fn register(&self, reg: Registration) {
        let mut g = self.inner.regs.lock().await;
        g.insert(reg.id.clone(), reg);
    }

    /// Snapshot of all registrations.
    pub async fn list(&self) -> Vec<Registration> {
        self.inner.regs.lock().await.values().cloned().collect()
    }

    /// Disable a trigger by id.
    pub async fn disable(&self, id: &str) {
        if let Some(r) = self.inner.regs.lock().await.get_mut(id) {
            r.enabled = false;
        }
    }

    /// Evaluate the registry against `now_unix_seconds` and return any
    /// triggers that should fire. Cron triggers fire once per minute window
    /// after they cross a scheduled time. Last-fire is updated on each fire.
    pub async fn tick(&self, now_unix_seconds: i64) -> Vec<FireAction> {
        let mut out = Vec::new();
        let mut regs = self.inner.regs.lock().await;
        for r in regs.values_mut() {
            if !r.enabled {
                continue;
            }
            if let Trigger::Cron { expression } = &r.trigger {
                let Ok(schedule) = cron::Schedule::from_str(expression) else {
                    continue;
                };
                let last_fire_secs = r
                    .last_fired_at
                    .as_deref()
                    .and_then(|s| {
                        time::OffsetDateTime::parse(
                            s,
                            &time::format_description::well_known::Rfc3339,
                        )
                        .ok()
                        .map(|t| t.unix_timestamp())
                    })
                    .unwrap_or(0);
                // Find any scheduled time in (last_fire_secs, now_unix_seconds].
                let after = chrono::DateTime::<chrono::Utc>::from_timestamp(last_fire_secs, 0)
                    .unwrap_or(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH);
                let now_dt = chrono::DateTime::<chrono::Utc>::from_timestamp(now_unix_seconds, 0)
                    .unwrap_or(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH);
                if let Some(next) = schedule.after(&after).next() {
                    if next <= now_dt {
                        out.push(FireAction::Fire {
                            trigger_id: r.id.clone(),
                            workflow_name: r.workflow_name.clone(),
                        });
                        r.last_fired_at = Some(now_iso(now_dt));
                    }
                }
            }
        }
        out
    }

    /// Drive the scheduler on a fixed interval until the watch channel
    /// flips. Each tick invokes `on_fire` for any fired action.
    pub async fn run<F>(
        &self,
        interval: std::time::Duration,
        mut on_fire: F,
        mut shutdown: watch::Receiver<bool>,
    ) where
        F: FnMut(FireAction) + Send,
    {
        loop {
            if *shutdown.borrow() {
                return;
            }
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            for action in self.tick(now).await {
                on_fire(action);
            }
            tokio::select! {
                _ = tokio::time::sleep(interval) => {}
                _ = shutdown.changed() => return,
            }
        }
    }
}

/// Spawn a workflow_run row in response to a fired trigger.
/// Returns the workflow_run id. The caller (e.g. an executor service) is
/// responsible for picking it up and running it through `actant-flow::Runner`.
pub async fn spawn_workflow_run(
    storage: &actant_storage::Storage,
    workspace_id: &actant_core::WorkspaceId,
    workflow_id: &actant_core::WorkflowId,
    trigger_id: &str,
) -> Result<actant_core::WorkflowRunId, actant_core::ActantError> {
    let run_id = actant_core::WorkflowRunId::new();
    let now = actant_core::now_rfc3339();
    // Also write a synthetic agent_event so the chronicle reflects the spawn.
    let payload = serde_json::json!({
        "trigger_id": trigger_id,
        "workflow_id": workflow_id.as_str(),
        "workflow_run_id": run_id.as_str(),
    });
    let payload_canon = actant_core::canonical_json(&payload);
    let payload_hash = actant_core::sha256_hex(payload_canon.as_bytes());
    let prev = storage
        .last_event_hash(workspace_id, None)
        .await?
        .unwrap_or_else(|| "0".repeat(64));
    let event_hash = actant_core::chain_hash(&prev, &payload_hash);
    let event = actant_core::AgentEvent {
        id: actant_core::EventId::new(),
        workspace_id: workspace_id.clone(),
        actor_id: actant_core::ActorId::from_string("act_system".to_string()),
        session_id: None,
        parent_event_id: None,
        event_type: "workflow_run_started".into(),
        causality_kind: actant_core::CausalityKind::Control,
        sensitivity: actant_core::Sensitivity::Low,
        authority_scope_id: None,
        payload_ref: None,
        payload_inline: Some(payload_canon),
        payload_hash,
        event_hash,
        created_at: now.clone(),
        model_call_id: None,
        tool_call_id: None,
        workflow_run_id: Some(run_id.clone()),
        memory_id: None,
        artifact_id: None,
        command_id: None,
        effect_id: None,
    };
    storage.append_event(&event).await?;
    // workflow_run row.
    sqlx::query(
        "INSERT INTO workflow_run (id, workflow_id, workspace_id, status, started_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(run_id.as_str())
    .bind(workflow_id.as_str())
    .bind(workspace_id.as_str())
    .bind("running")
    .bind(&now)
    .execute(storage.pool())
    .await
    .map_err(|e| actant_core::ActantError::Storage(e.to_string()))?;
    Ok(run_id)
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

fn now_iso(dt: chrono::DateTime<chrono::Utc>) -> String {
    time::OffsetDateTime::from_unix_timestamp(dt.timestamp())
        .expect("unix")
        .format(&time::format_description::well_known::Rfc3339)
        .expect("rfc3339")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cron_validates() {
        let t = Trigger::Cron {
            expression: "0 0 7 * * * *".into(),
        };
        assert!(t.validate());
        let bad = Trigger::Cron {
            expression: "not a cron expr".into(),
        };
        assert!(!bad.validate());
    }

    #[test]
    fn webhook_validates() {
        let t = Trigger::Webhook { path: "/x".into() };
        assert!(t.validate());
    }

    #[tokio::test]
    async fn scheduler_fires_a_past_due_cron() {
        let s = Scheduler::new();
        s.register(Registration {
            id: "t1".into(),
            name: "daily".into(),
            trigger: Trigger::Cron {
                expression: "0 0 0 * * * *".into(), // daily at 00:00 UTC
            },
            workflow_name: "daily-digest".into(),
            last_fired_at: None,
            enabled: true,
        })
        .await;
        // Tick at "now" — scheduler should find at least one past-due fire
        // (because last_fired_at is None and the cron has fired countless
        // times in the past).
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let fires = s.tick(now).await;
        assert!(!fires.is_empty(), "expected a cron fire");
    }

    #[tokio::test]
    async fn spawn_writes_workflow_run_and_event() {
        use actant_core::*;
        use actant_storage::{Storage, StorageConfig};

        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: "t".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        s.insert_workspace(&ws).await.unwrap();
        s.insert_actor(&Actor {
            id: ActorId::from_string("act_system".to_string()),
            workspace_id: ws.id.clone(),
            kind: ActorKind::System,
            display_name: "system".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        })
        .await
        .unwrap();
        // workflow row (FK target)
        sqlx::query(
            "INSERT INTO workflow (id, workspace_id, name, owner_actor_id, version,
                                   status, definition_ref, definition_hash, created_at)
             VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind("wf_1")
        .bind(ws.id.as_str())
        .bind("test")
        .bind("act_system")
        .bind(1i64)
        .bind("active")
        .bind("art:def")
        .bind("h")
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .unwrap();

        let run_id = spawn_workflow_run(
            &s,
            &ws.id,
            &WorkflowId::from_string("wf_1".to_string()),
            "t1",
        )
        .await
        .unwrap();
        // Chronicle has the workflow_run_started event.
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM agent_event WHERE event_type='workflow_run_started'",
        )
        .fetch_one(s.pool())
        .await
        .unwrap();
        assert_eq!(n, 1);
        // workflow_run row exists.
        let (status,): (String,) = sqlx::query_as("SELECT status FROM workflow_run WHERE id = ?")
            .bind(run_id.as_str())
            .fetch_one(s.pool())
            .await
            .unwrap();
        assert_eq!(status, "running");
    }

    #[tokio::test]
    async fn disabled_trigger_does_not_fire() {
        let s = Scheduler::new();
        s.register(Registration {
            id: "t1".into(),
            name: "x".into(),
            trigger: Trigger::Cron {
                expression: "0 0 0 * * * *".into(),
            },
            workflow_name: "wf".into(),
            last_fired_at: None,
            enabled: false,
        })
        .await;
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let fires = s.tick(now).await;
        assert!(fires.is_empty());
    }
}
