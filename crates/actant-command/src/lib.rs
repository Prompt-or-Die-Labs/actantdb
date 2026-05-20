//! actant-command — typed command engine.
//!
//! Hosts the alpha command set from `/specs/10-alpha-demo.md` §1–11:
//! `create_session`, `append_user_message`, `append_agent_message`,
//! `request_tool_call`, `approve_tool_call`, `deny_tool_call`,
//! `record_tool_result`, `propose_memory`, `approve_memory`, `reject_memory`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::*;
use actant_policy::{alpha_demo_policy, evaluate, GuardInput, PolicyDoc, Verdict};
use actant_storage::{PgStorage, Storage, StorageBackend};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Result of dispatching a command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutcome {
    /// Command id assigned.
    pub command_id: CommandId,
    /// Chronicle event written.
    pub event_id: Option<EventId>,
    /// Free-form result payload.
    pub result: serde_json::Value,
}

/// The command engine.
///
/// Wraps a [`StorageBackend`] so the same engine can be constructed against
/// either the SQLite-backed [`Storage`] or the Postgres-backed [`PgStorage`].
#[derive(Clone)]
pub struct Engine {
    backend: StorageBackend,
    policy: PolicyDoc,
}

impl Engine {
    /// Construct an engine wrapping a SQLite [`Storage`] with the default
    /// alpha-demo policy. Equivalent to `Engine::from_backend(storage.into())`.
    pub fn new(storage: Storage) -> Self {
        Self::from_backend(StorageBackend::Sqlite(storage))
    }

    /// Construct with an explicit policy against a SQLite [`Storage`].
    pub fn with_policy(storage: Storage, policy: PolicyDoc) -> Self {
        Self {
            backend: StorageBackend::Sqlite(storage),
            policy,
        }
    }

    /// Construct an engine wrapping a [`PgStorage`] with the default policy.
    pub fn postgres(pg: PgStorage) -> Self {
        Self::from_backend(StorageBackend::Postgres(pg))
    }

    /// Construct an engine from any [`StorageBackend`] with the default policy.
    pub fn from_backend(backend: StorageBackend) -> Self {
        Self {
            backend,
            policy: alpha_demo_policy(),
        }
    }

    /// Construct an engine from any [`StorageBackend`] with an explicit policy.
    pub fn from_backend_with_policy(backend: StorageBackend, policy: PolicyDoc) -> Self {
        Self { backend, policy }
    }

    /// Underlying SQLite storage handle.
    ///
    /// **Panics** if the engine was constructed against a Postgres backend.
    /// Callers that need to be backend-agnostic should use [`Self::backend`]
    /// or [`Self::storage_opt`] instead. Every existing call site is
    /// SQLite-only today, so this preserves the prior signature.
    pub fn storage(&self) -> &Storage {
        match &self.backend {
            StorageBackend::Sqlite(s) => s,
            StorageBackend::Postgres(_) => panic!(
                "Engine::storage() called on a Postgres-backed Engine; use \
                 Engine::backend() or Engine::storage_opt() instead. {}",
                actant_storage::PG_NOT_IMPLEMENTED_HINT
            ),
        }
    }

    /// Underlying SQLite storage handle, if the backend is SQLite.
    pub fn storage_opt(&self) -> Option<&Storage> {
        self.backend.as_sqlite()
    }

    /// Underlying backend handle (works for both SQLite and Postgres).
    pub fn backend(&self) -> &StorageBackend {
        &self.backend
    }

    /// Active policy.
    pub fn policy(&self) -> &PolicyDoc {
        &self.policy
    }

    /// Dispatch a command by type name + JSON input.
    pub async fn dispatch(
        &self,
        workspace_id: &WorkspaceId,
        actor_id: &ActorId,
        command_type: &str,
        input: serde_json::Value,
        idempotency_key: Option<&str>,
    ) -> Result<CommandOutcome, ActantError> {
        if self.backend.get_workspace(workspace_id).await?.is_none() {
            self.backend
                .insert_workspace(&Workspace {
                    id: workspace_id.clone(),
                    name: workspace_id.as_str().to_string(),
                    created_at: now_rfc3339(),
                    archived_at: None,
                })
                .await?;
        }
        if self.backend.get_actor(actor_id).await?.is_none() {
            self.backend
                .insert_actor(&Actor {
                    id: actor_id.clone(),
                    workspace_id: workspace_id.clone(),
                    kind: ActorKind::Human,
                    display_name: actor_id.as_str().to_string(),
                    created_at: now_rfc3339(),
                    disabled_at: None,
                })
                .await?;
        }

        if let Some(key) = idempotency_key {
            if let Some(prior) = self.backend.idempotency_lookup(workspace_id, key).await? {
                return Ok(CommandOutcome {
                    command_id: CommandId::from_string(prior),
                    event_id: None,
                    result: serde_json::json!({"idempotent_replay": true}),
                });
            }
        }

        let canonical = canonical_json(&input);
        let input_hash = sha256_hex(canonical.as_bytes());

        let cmd = CommandRecord {
            id: CommandId::new(),
            workspace_id: workspace_id.clone(),
            actor_id: actor_id.clone(),
            session_id: None,
            command_type: command_type.into(),
            input_inline: Some(canonical.clone()),
            input_hash: input_hash.clone(),
            policy_id: None,
            status: CommandStatus::Received,
            error: None,
            created_at: now_rfc3339(),
            committed_at: None,
        };
        self.backend.insert_command(&cmd).await?;

        let result = match command_type {
            "create_session" => {
                self.create_session(workspace_id, actor_id, &cmd, &input)
                    .await
            }
            "append_user_message" => {
                self.append_message(
                    workspace_id,
                    actor_id,
                    &cmd,
                    &input,
                    "user",
                    "user_message_received",
                )
                .await
            }
            "append_agent_message" => {
                self.append_message(
                    workspace_id,
                    actor_id,
                    &cmd,
                    &input,
                    "agent",
                    "agent_message",
                )
                .await
            }
            "request_tool_call" => {
                self.request_tool_call(workspace_id, actor_id, &cmd, &input)
                    .await
            }
            "approve_tool_call" => {
                self.approve_tool_call(workspace_id, actor_id, &cmd, &input)
                    .await
            }
            "deny_tool_call" => {
                self.deny_tool_call(workspace_id, actor_id, &cmd, &input)
                    .await
            }
            "record_tool_result" => {
                self.record_tool_result(workspace_id, actor_id, &cmd, &input)
                    .await
            }
            "propose_memory" => {
                self.propose_memory(workspace_id, actor_id, &cmd, &input)
                    .await
            }
            "approve_memory" => {
                self.approve_memory(workspace_id, actor_id, &cmd, &input)
                    .await
            }
            "reject_memory" => {
                self.reject_memory(workspace_id, actor_id, &cmd, &input)
                    .await
            }
            other => Err(ActantError::InvalidInput(format!(
                "unknown command_type: {other}"
            ))),
        }?;

        if let Some(key) = idempotency_key {
            let _ = self
                .backend
                .idempotency_record(
                    workspace_id,
                    actor_id,
                    key,
                    command_type,
                    &input_hash,
                    Some(cmd.id.as_str()),
                )
                .await;
        }

        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    async fn append_chronicle(
        &self,
        workspace_id: &WorkspaceId,
        actor_id: &ActorId,
        session_id: Option<&SessionId>,
        event_type: &str,
        causality_kind: CausalityKind,
        sensitivity: Sensitivity,
        payload: &serde_json::Value,
        backrefs: EventBackrefs,
    ) -> Result<EventId, ActantError> {
        let payload_canon = canonical_json(payload);
        let payload_hash = sha256_hex(payload_canon.as_bytes());
        let prev = self
            .backend
            .last_event_hash(workspace_id, session_id)
            .await?
            .unwrap_or_else(|| "0".repeat(64));
        let event_hash = chain_hash(&prev, &payload_hash);
        let e = AgentEvent {
            id: EventId::new(),
            workspace_id: workspace_id.clone(),
            actor_id: actor_id.clone(),
            session_id: session_id.cloned(),
            parent_event_id: None,
            event_type: event_type.into(),
            causality_kind,
            sensitivity,
            authority_scope_id: None,
            payload_ref: None,
            payload_inline: Some(payload_canon),
            payload_hash,
            event_hash,
            created_at: now_rfc3339(),
            model_call_id: backrefs.model_call_id,
            tool_call_id: backrefs.tool_call_id,
            workflow_run_id: backrefs.workflow_run_id,
            memory_id: backrefs.memory_id,
            artifact_id: backrefs.artifact_id,
            command_id: backrefs.command_id,
            effect_id: backrefs.effect_id,
        };
        let id = e.id.clone();
        self.backend.append_event(&e).await?;
        Ok(id)
    }

    async fn create_session(
        &self,
        ws: &WorkspaceId,
        actor: &ActorId,
        cmd: &CommandRecord,
        input: &Value,
    ) -> Result<CommandOutcome, ActantError> {
        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .map(String::from);
        let agent = input
            .get("agent_actor_id")
            .and_then(|v| v.as_str())
            .map(|s| ActorId::from_string(s.to_string()));
        if let Some(agent_id) = &agent {
            if self.backend.get_actor(agent_id).await?.is_none() {
                self.backend
                    .insert_actor(&Actor {
                        id: agent_id.clone(),
                        workspace_id: ws.clone(),
                        kind: ActorKind::Agent,
                        display_name: agent_id.as_str().to_string(),
                        created_at: now_rfc3339(),
                        disabled_at: None,
                    })
                    .await?;
            }
        }
        let session_id = input
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| SessionId::from_string(s.to_string()))
            .unwrap_or_default();
        let exists = session_exists(&self.backend, ws, &session_id).await?;
        if exists {
            return Ok(CommandOutcome {
                command_id: cmd.id.clone(),
                event_id: None,
                result: serde_json::json!({"session_id": session_id.as_str()}),
            });
        }
        let session = Session {
            id: session_id,
            workspace_id: ws.clone(),
            title,
            initiator_actor_id: actor.clone(),
            agent_actor_id: agent,
            status: SessionStatus::Active,
            created_at: now_rfc3339(),
            closed_at: None,
        };
        self.backend.insert_session(&session).await?;
        let event_id = self
            .append_chronicle(
                ws,
                actor,
                Some(&session.id),
                "session_created",
                CausalityKind::Control,
                Sensitivity::Low,
                &serde_json::json!({"session_id": session.id.as_str()}),
                EventBackrefs::with_command(cmd.id.clone()),
            )
            .await?;
        Ok(CommandOutcome {
            command_id: cmd.id.clone(),
            event_id: Some(event_id),
            result: serde_json::json!({"session_id": session.id.as_str()}),
        })
    }

    async fn append_message(
        &self,
        ws: &WorkspaceId,
        actor: &ActorId,
        cmd: &CommandRecord,
        input: &Value,
        role: &str,
        event_type: &str,
    ) -> Result<CommandOutcome, ActantError> {
        let session_id = SessionId::from_string(required_str(input, "session_id")?.to_string());
        let text = required_str(input, "text")?.to_string();
        let body_hash = sha256_hex(text.as_bytes());
        let msg_id = MessageId::new();
        insert_message(
            &self.backend,
            &msg_id,
            &session_id,
            ws,
            actor,
            role,
            &text,
            &body_hash,
        )
        .await?;
        let event_id = self
            .append_chronicle(
                ws,
                actor,
                Some(&session_id),
                event_type,
                CausalityKind::Observation,
                Sensitivity::Low,
                &serde_json::json!({"message_id": msg_id.as_str(), "text": text}),
                EventBackrefs::with_command(cmd.id.clone()),
            )
            .await?;
        Ok(CommandOutcome {
            command_id: cmd.id.clone(),
            event_id: Some(event_id),
            result: serde_json::json!({"message_id": msg_id.as_str()}),
        })
    }

    async fn request_tool_call(
        &self,
        ws: &WorkspaceId,
        actor: &ActorId,
        cmd: &CommandRecord,
        input: &Value,
    ) -> Result<CommandOutcome, ActantError> {
        let session_id = SessionId::from_string(required_str(input, "session_id")?.to_string());
        let tool_name = required_str(input, "tool_name")?.to_string();
        let arguments = input
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let arguments_canon = canonical_json(&arguments);
        let arguments_hash = sha256_hex(arguments_canon.as_bytes());

        let tool_id = upsert_tool(&self.backend, ws, &tool_name).await?;

        let v = evaluate(
            &self.policy,
            &GuardInput {
                actor_id: actor,
                tool: &tool_name,
                arguments_json: &arguments_canon,
                risk_level: risk_for(&tool_name, &self.policy),
                sensitivity: Sensitivity::Low,
            },
        );

        let tool_call_id = ToolCallId::new();
        let status = match &v {
            Verdict::Allow { .. } | Verdict::Constrain { .. } => ToolCallStatus::Approved,
            Verdict::RequireApproval { .. } => ToolCallStatus::PendingApproval,
            Verdict::Block { reason } => return Err(ActantError::PermissionDenied(reason.clone())),
            Verdict::Halt { reason } => return Err(ActantError::PolicyHalt(reason.clone())),
        };

        insert_tool_call(
            &self.backend,
            &tool_call_id,
            ws,
            &session_id,
            actor,
            &tool_id,
            &arguments_canon,
            &arguments_hash,
            &status,
            &risk_for(&tool_name, &self.policy),
        )
        .await?;

        // Now (optionally) create the approval_request and link it.
        let approval_request_id = if matches!(v, Verdict::RequireApproval { .. }) {
            let ar_id = ApprovalRequestId::new();
            insert_approval_request(
                &self.backend,
                &ar_id,
                ws,
                &tool_call_id,
                actor,
                &risk_for(&tool_name, &self.policy),
                &tool_name,
                &format!("{tool_name}: {arguments_canon}"),
            )
            .await?;
            set_tool_call_approval_request(&self.backend, &tool_call_id, &ar_id).await?;
            Some(ar_id)
        } else {
            None
        };
        let _ = approval_request_id;

        let verdict_payload = serde_json::to_value(&v).unwrap_or(serde_json::json!({}));
        let event_id = self
            .append_chronicle(
                ws,
                actor,
                Some(&session_id),
                "tool_call_requested",
                CausalityKind::Intent,
                Sensitivity::Low,
                &serde_json::json!({
                    "tool_call_id": tool_call_id.as_str(),
                    "tool": tool_name,
                    "arguments": arguments,
                    "verdict": verdict_payload,
                }),
                EventBackrefs {
                    command_id: Some(cmd.id.clone()),
                    tool_call_id: Some(tool_call_id.clone()),
                    ..Default::default()
                },
            )
            .await?;
        Ok(CommandOutcome {
            command_id: cmd.id.clone(),
            event_id: Some(event_id),
            result: serde_json::json!({
                "tool_call_id": tool_call_id.as_str(),
                "status": json_enum(&status),
                "verdict": verdict_payload,
            }),
        })
    }

    async fn approve_tool_call(
        &self,
        ws: &WorkspaceId,
        actor: &ActorId,
        cmd: &CommandRecord,
        input: &Value,
    ) -> Result<CommandOutcome, ActantError> {
        let tool_call_id = required_str(input, "tool_call_id")?.to_string();
        let scope = input
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("once")
            .to_string();
        let session_id = session_for_tool_call(&self.backend, &tool_call_id).await?;

        approve_pending_tool_call(&self.backend, &tool_call_id, actor, &scope).await?;

        let event_id = self
            .append_chronicle(
                ws,
                actor,
                session_id.as_ref(),
                "tool_call_approved",
                CausalityKind::Control,
                Sensitivity::Low,
                &serde_json::json!({
                    "tool_call_id": tool_call_id,
                    "scope": scope,
                    "approver": actor.as_str(),
                }),
                EventBackrefs {
                    command_id: Some(cmd.id.clone()),
                    tool_call_id: Some(ToolCallId::from_string(tool_call_id.clone())),
                    ..Default::default()
                },
            )
            .await?;
        Ok(CommandOutcome {
            command_id: cmd.id.clone(),
            event_id: Some(event_id),
            result: serde_json::json!({"approved": tool_call_id, "scope": scope}),
        })
    }

    async fn deny_tool_call(
        &self,
        ws: &WorkspaceId,
        actor: &ActorId,
        cmd: &CommandRecord,
        input: &Value,
    ) -> Result<CommandOutcome, ActantError> {
        let tool_call_id = required_str(input, "tool_call_id")?.to_string();
        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("denied")
            .to_string();
        let session_id = session_for_tool_call(&self.backend, &tool_call_id).await?;

        deny_pending_tool_call(&self.backend, &tool_call_id, actor, &reason).await?;

        let event_id = self
            .append_chronicle(
                ws,
                actor,
                session_id.as_ref(),
                "tool_call_denied",
                CausalityKind::Control,
                Sensitivity::Low,
                &serde_json::json!({"tool_call_id": tool_call_id, "reason": reason}),
                EventBackrefs {
                    command_id: Some(cmd.id.clone()),
                    tool_call_id: Some(ToolCallId::from_string(tool_call_id.clone())),
                    ..Default::default()
                },
            )
            .await?;
        Ok(CommandOutcome {
            command_id: cmd.id.clone(),
            event_id: Some(event_id),
            result: serde_json::json!({"denied": tool_call_id, "reason": reason}),
        })
    }

    async fn record_tool_result(
        &self,
        ws: &WorkspaceId,
        actor: &ActorId,
        cmd: &CommandRecord,
        input: &Value,
    ) -> Result<CommandOutcome, ActantError> {
        let tool_call_id = required_str(input, "tool_call_id")?.to_string();
        let result = input
            .get("result")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let result_canon = canonical_json(&result);
        let result_hash = sha256_hex(result_canon.as_bytes());
        let session_id = session_for_tool_call(&self.backend, &tool_call_id).await?;

        complete_tool_call(&self.backend, &tool_call_id, &result_canon, &result_hash).await?;

        let event_id = self
            .append_chronicle(
                ws,
                actor,
                session_id.as_ref(),
                "tool_call_finished",
                CausalityKind::Effect,
                Sensitivity::Low,
                &serde_json::json!({"tool_call_id": tool_call_id, "result": result}),
                EventBackrefs {
                    command_id: Some(cmd.id.clone()),
                    tool_call_id: Some(ToolCallId::from_string(tool_call_id.clone())),
                    ..Default::default()
                },
            )
            .await?;
        Ok(CommandOutcome {
            command_id: cmd.id.clone(),
            event_id: Some(event_id),
            result: serde_json::json!({"tool_call_id": tool_call_id, "result_hash": result_hash}),
        })
    }

    async fn propose_memory(
        &self,
        ws: &WorkspaceId,
        actor: &ActorId,
        cmd: &CommandRecord,
        input: &Value,
    ) -> Result<CommandOutcome, ActantError> {
        let text = required_str(input, "text")?.to_string();
        let category = input
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("fact")
            .to_string();
        let confidence = input
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
        let sensitivity_s = input
            .get("sensitivity")
            .and_then(|v| v.as_str())
            .unwrap_or("low")
            .to_string();
        let source_event_ids = input
            .get("source_event_ids")
            .cloned()
            .unwrap_or(serde_json::json!([]));
        let mc_id = MemoryCandidateId::new();
        insert_memory_candidate(
            &self.backend,
            &mc_id,
            ws,
            actor,
            &source_event_ids.to_string(),
            &text,
            &category,
            confidence,
            &sensitivity_s,
        )
        .await?;

        let event_id = self
            .append_chronicle(
                ws,
                actor,
                None,
                "memory_proposed",
                CausalityKind::Observation,
                Sensitivity::Low,
                &serde_json::json!({
                    "memory_candidate_id": mc_id.as_str(),
                    "text": text,
                    "confidence": confidence,
                }),
                EventBackrefs::with_command(cmd.id.clone()),
            )
            .await?;
        Ok(CommandOutcome {
            command_id: cmd.id.clone(),
            event_id: Some(event_id),
            result: serde_json::json!({"memory_candidate_id": mc_id.as_str()}),
        })
    }

    async fn approve_memory(
        &self,
        ws: &WorkspaceId,
        actor: &ActorId,
        cmd: &CommandRecord,
        input: &Value,
    ) -> Result<CommandOutcome, ActantError> {
        let mc_id = required_str(input, "memory_candidate_id")?.to_string();
        let (_, text, category, confidence, sensitivity_s, source_event_ids) =
            memory_candidate(&self.backend, &mc_id)
                .await?
                .ok_or_else(|| ActantError::NotFound(format!("memory_candidate {mc_id}")))?;
        let mem_id = MemoryId::new();
        insert_memory(
            &self.backend,
            &mem_id,
            ws,
            &text,
            &category,
            &sensitivity_s,
            confidence,
            &mc_id,
            &source_event_ids,
        )
        .await?;
        approve_memory_candidate(&self.backend, &mc_id).await?;
        let event_id = self
            .append_chronicle(
                ws,
                actor,
                None,
                "memory_approved",
                CausalityKind::Control,
                Sensitivity::Low,
                &serde_json::json!({
                    "memory_id": mem_id.as_str(),
                    "memory_candidate_id": mc_id,
                }),
                EventBackrefs {
                    command_id: Some(cmd.id.clone()),
                    memory_id: Some(mem_id.clone()),
                    ..Default::default()
                },
            )
            .await?;
        Ok(CommandOutcome {
            command_id: cmd.id.clone(),
            event_id: Some(event_id),
            result: serde_json::json!({"memory_id": mem_id.as_str()}),
        })
    }

    async fn reject_memory(
        &self,
        ws: &WorkspaceId,
        actor: &ActorId,
        cmd: &CommandRecord,
        input: &Value,
    ) -> Result<CommandOutcome, ActantError> {
        let mc_id = required_str(input, "memory_candidate_id")?.to_string();
        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("rejected")
            .to_string();
        reject_memory_candidate(&self.backend, &mc_id, &reason).await?;
        let event_id = self
            .append_chronicle(
                ws,
                actor,
                None,
                "memory_rejected",
                CausalityKind::Control,
                Sensitivity::Low,
                &serde_json::json!({"memory_candidate_id": mc_id, "reason": reason}),
                EventBackrefs::with_command(cmd.id.clone()),
            )
            .await?;
        Ok(CommandOutcome {
            command_id: cmd.id.clone(),
            event_id: Some(event_id),
            result: serde_json::json!({"memory_candidate_id": mc_id}),
        })
    }
}

#[derive(Debug, Clone, Default)]
struct EventBackrefs {
    command_id: Option<CommandId>,
    tool_call_id: Option<ToolCallId>,
    model_call_id: Option<ModelCallId>,
    workflow_run_id: Option<WorkflowRunId>,
    memory_id: Option<MemoryId>,
    artifact_id: Option<ArtifactId>,
    effect_id: Option<EffectId>,
}

impl EventBackrefs {
    fn with_command(cmd: CommandId) -> Self {
        Self {
            command_id: Some(cmd),
            ..Default::default()
        }
    }
}

fn required_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, ActantError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ActantError::InvalidInput(format!("missing required {key}")))
}

fn json_enum<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v)
        .unwrap_or_else(|_| "\"\"".into())
        .trim_matches('"')
        .to_string()
}

fn risk_for(tool: &str, policy: &PolicyDoc) -> RiskLevel {
    policy
        .tools
        .iter()
        .find(|t| t.tool == tool)
        .map(|t| t.risk_level)
        .unwrap_or(RiskLevel::Low)
}

async fn session_exists(
    backend: &StorageBackend,
    ws: &WorkspaceId,
    session_id: &SessionId,
) -> Result<bool, ActantError> {
    let row: Option<(i64,)> = match backend {
        StorageBackend::Sqlite(s) => {
            sqlx::query_as("SELECT 1 FROM session WHERE id = ? AND workspace_id = ? LIMIT 1")
                .bind(session_id.as_str())
                .bind(ws.as_str())
                .fetch_optional(s.pool())
                .await
        }
        StorageBackend::Postgres(p) => {
            sqlx::query_as("SELECT 1 FROM session WHERE id = $1 AND workspace_id = $2 LIMIT 1")
                .bind(session_id.as_str())
                .bind(ws.as_str())
                .fetch_optional(p.pool())
                .await
        }
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(row.is_some())
}

#[allow(clippy::too_many_arguments)]
async fn insert_message(
    backend: &StorageBackend,
    msg_id: &MessageId,
    session_id: &SessionId,
    ws: &WorkspaceId,
    actor: &ActorId,
    role: &str,
    text: &str,
    body_hash: &str,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => sqlx::query(
            "INSERT INTO message
                    (id, session_id, workspace_id, author_actor_id, role,
                     body_ref, body_text, body_hash, created_at)
                 VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(msg_id.as_str())
        .bind(session_id.as_str())
        .bind(ws.as_str())
        .bind(actor.as_str())
        .bind(role)
        .bind::<Option<&str>>(None)
        .bind(text)
        .bind(body_hash)
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .map(|_| ()),
        StorageBackend::Postgres(p) => sqlx::query(
            "INSERT INTO message
                    (id, session_id, workspace_id, author_actor_id, role,
                     body_ref, body_text, body_hash, created_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(msg_id.as_str())
        .bind(session_id.as_str())
        .bind(ws.as_str())
        .bind(actor.as_str())
        .bind(role)
        .bind::<Option<&str>>(None)
        .bind(text)
        .bind(body_hash)
        .bind(now_rfc3339())
        .execute(p.pool())
        .await
        .map(|_| ()),
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

async fn upsert_tool(
    backend: &StorageBackend,
    ws: &WorkspaceId,
    name: &str,
) -> Result<ToolId, ActantError> {
    let row: Option<(String,)> = match backend {
        StorageBackend::Sqlite(s) => {
            sqlx::query_as("SELECT id FROM tool WHERE workspace_id = ? AND name = ?")
                .bind(ws.as_str())
                .bind(name)
                .fetch_optional(s.pool())
                .await
        }
        StorageBackend::Postgres(p) => {
            sqlx::query_as("SELECT id FROM tool WHERE workspace_id = $1 AND name = $2")
                .bind(ws.as_str())
                .bind(name)
                .fetch_optional(p.pool())
                .await
        }
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    if let Some((id,)) = row {
        return Ok(ToolId::from_string(id));
    }
    let id = ToolId::new();
    match backend {
        StorageBackend::Sqlite(s) => sqlx::query(
            "INSERT INTO tool (id, workspace_id, name, kind, required_permission,
                                   default_risk_level, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.as_str())
        .bind(ws.as_str())
        .bind(name)
        .bind(name.split('.').next().unwrap_or("custom"))
        .bind(name)
        .bind("medium")
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .map(|_| ()),
        StorageBackend::Postgres(p) => sqlx::query(
            "INSERT INTO tool (id, workspace_id, name, kind, required_permission,
                                   default_risk_level, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id.as_str())
        .bind(ws.as_str())
        .bind(name)
        .bind(name.split('.').next().unwrap_or("custom"))
        .bind(name)
        .bind("medium")
        .bind(now_rfc3339())
        .execute(p.pool())
        .await
        .map(|_| ()),
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
async fn insert_tool_call(
    backend: &StorageBackend,
    tool_call_id: &ToolCallId,
    ws: &WorkspaceId,
    session_id: &SessionId,
    actor: &ActorId,
    tool_id: &ToolId,
    arguments_canon: &str,
    arguments_hash: &str,
    status: &ToolCallStatus,
    risk: &RiskLevel,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => sqlx::query(
            "INSERT INTO tool_call
                    (id, workspace_id, session_id, requested_by_actor_id,
                     tool_id, schema_version, arguments_inline, arguments_hash,
                     status, risk_level, created_at)
                 VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(tool_call_id.as_str())
        .bind(ws.as_str())
        .bind(session_id.as_str())
        .bind(actor.as_str())
        .bind(tool_id.as_str())
        .bind(1i64)
        .bind(arguments_canon)
        .bind(arguments_hash)
        .bind(json_enum(status))
        .bind(json_enum(risk))
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .map(|_| ()),
        StorageBackend::Postgres(p) => sqlx::query(
            "INSERT INTO tool_call
                    (id, workspace_id, session_id, requested_by_actor_id,
                     tool_id, schema_version, arguments_inline, arguments_hash,
                     status, risk_level, created_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(tool_call_id.as_str())
        .bind(ws.as_str())
        .bind(session_id.as_str())
        .bind(actor.as_str())
        .bind(tool_id.as_str())
        .bind(1i64)
        .bind(arguments_canon)
        .bind(arguments_hash)
        .bind(json_enum(status))
        .bind(json_enum(risk))
        .bind(now_rfc3339())
        .execute(p.pool())
        .await
        .map(|_| ()),
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_approval_request(
    backend: &StorageBackend,
    approval_request_id: &ApprovalRequestId,
    ws: &WorkspaceId,
    tool_call_id: &ToolCallId,
    actor: &ActorId,
    risk: &RiskLevel,
    permission: &str,
    summary: &str,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => sqlx::query(
            "INSERT INTO approval_request
                    (id, workspace_id, tool_call_id, requested_by_actor_id,
                     risk_level, required_permission, summary,
                     status, created_at)
                 VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(approval_request_id.as_str())
        .bind(ws.as_str())
        .bind(tool_call_id.as_str())
        .bind(actor.as_str())
        .bind(json_enum(risk))
        .bind(permission)
        .bind(summary)
        .bind("pending")
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .map(|_| ()),
        StorageBackend::Postgres(p) => sqlx::query(
            "INSERT INTO approval_request
                    (id, workspace_id, tool_call_id, requested_by_actor_id,
                     risk_level, required_permission, summary,
                     status, created_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(approval_request_id.as_str())
        .bind(ws.as_str())
        .bind(tool_call_id.as_str())
        .bind(actor.as_str())
        .bind(json_enum(risk))
        .bind(permission)
        .bind(summary)
        .bind("pending")
        .bind(now_rfc3339())
        .execute(p.pool())
        .await
        .map(|_| ()),
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

async fn set_tool_call_approval_request(
    backend: &StorageBackend,
    tool_call_id: &ToolCallId,
    approval_request_id: &ApprovalRequestId,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => {
            sqlx::query("UPDATE tool_call SET approval_request_id = ? WHERE id = ?")
                .bind(approval_request_id.as_str())
                .bind(tool_call_id.as_str())
                .execute(s.pool())
                .await
                .map(|_| ())
        }
        StorageBackend::Postgres(p) => {
            sqlx::query("UPDATE tool_call SET approval_request_id = $1 WHERE id = $2")
                .bind(approval_request_id.as_str())
                .bind(tool_call_id.as_str())
                .execute(p.pool())
                .await
                .map(|_| ())
        }
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

async fn session_for_tool_call(
    backend: &StorageBackend,
    tool_call_id: &str,
) -> Result<Option<SessionId>, ActantError> {
    let row: Option<(Option<String>,)> = match backend {
        StorageBackend::Sqlite(s) => {
            sqlx::query_as("SELECT session_id FROM tool_call WHERE id = ?")
                .bind(tool_call_id)
                .fetch_optional(s.pool())
                .await
        }
        StorageBackend::Postgres(p) => {
            sqlx::query_as("SELECT session_id FROM tool_call WHERE id = $1")
                .bind(tool_call_id)
                .fetch_optional(p.pool())
                .await
        }
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(row.and_then(|(s,)| s.map(SessionId::from_string)))
}

async fn approve_pending_tool_call(
    backend: &StorageBackend,
    tool_call_id: &str,
    actor: &ActorId,
    scope: &str,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => {
            sqlx::query(
                "UPDATE approval_request
                 SET status='approved', approved_at=?, approved_by_actor_id=?, scope_granted=?
                 WHERE tool_call_id=? AND status='pending'",
            )
            .bind(now_rfc3339())
            .bind(actor.as_str())
            .bind(scope)
            .bind(tool_call_id)
            .execute(s.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
            sqlx::query("UPDATE tool_call SET status='approved' WHERE id=?")
                .bind(tool_call_id)
                .execute(s.pool())
                .await
                .map(|_| ())
        }
        StorageBackend::Postgres(p) => {
            sqlx::query(
                "UPDATE approval_request
                 SET status='approved', approved_at=$1, approved_by_actor_id=$2, scope_granted=$3
                 WHERE tool_call_id=$4 AND status='pending'",
            )
            .bind(now_rfc3339())
            .bind(actor.as_str())
            .bind(scope)
            .bind(tool_call_id)
            .execute(p.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
            sqlx::query("UPDATE tool_call SET status='approved' WHERE id=$1")
                .bind(tool_call_id)
                .execute(p.pool())
                .await
                .map(|_| ())
        }
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

async fn deny_pending_tool_call(
    backend: &StorageBackend,
    tool_call_id: &str,
    actor: &ActorId,
    reason: &str,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => {
            sqlx::query(
                "UPDATE approval_request
                 SET status='denied', approved_at=?, approved_by_actor_id=?, denied_reason=?
                 WHERE tool_call_id=? AND status='pending'",
            )
            .bind(now_rfc3339())
            .bind(actor.as_str())
            .bind(reason)
            .bind(tool_call_id)
            .execute(s.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
            sqlx::query("UPDATE tool_call SET status='denied' WHERE id=?")
                .bind(tool_call_id)
                .execute(s.pool())
                .await
                .map(|_| ())
        }
        StorageBackend::Postgres(p) => {
            sqlx::query(
                "UPDATE approval_request
                 SET status='denied', approved_at=$1, approved_by_actor_id=$2, denied_reason=$3
                 WHERE tool_call_id=$4 AND status='pending'",
            )
            .bind(now_rfc3339())
            .bind(actor.as_str())
            .bind(reason)
            .bind(tool_call_id)
            .execute(p.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
            sqlx::query("UPDATE tool_call SET status='denied' WHERE id=$1")
                .bind(tool_call_id)
                .execute(p.pool())
                .await
                .map(|_| ())
        }
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

async fn complete_tool_call(
    backend: &StorageBackend,
    tool_call_id: &str,
    result_canon: &str,
    result_hash: &str,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => sqlx::query(
            "UPDATE tool_call
                 SET status='completed', result_ref=?, result_hash=?, completed_at=?
                 WHERE id=?",
        )
        .bind(result_canon)
        .bind(result_hash)
        .bind(now_rfc3339())
        .bind(tool_call_id)
        .execute(s.pool())
        .await
        .map(|_| ()),
        StorageBackend::Postgres(p) => sqlx::query(
            "UPDATE tool_call
                 SET status='completed', result_ref=$1, result_hash=$2, completed_at=$3
                 WHERE id=$4",
        )
        .bind(result_canon)
        .bind(result_hash)
        .bind(now_rfc3339())
        .bind(tool_call_id)
        .execute(p.pool())
        .await
        .map(|_| ()),
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_memory_candidate(
    backend: &StorageBackend,
    id: &MemoryCandidateId,
    ws: &WorkspaceId,
    actor: &ActorId,
    source_event_ids: &str,
    text: &str,
    category: &str,
    confidence: f64,
    sensitivity: &str,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => sqlx::query(
            "INSERT INTO memory_candidate
                    (id, workspace_id, proposed_by_actor_id, source_event_ids,
                     text, category, confidence, sensitivity, status, created_at)
                 VALUES (?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(id.as_str())
        .bind(ws.as_str())
        .bind(actor.as_str())
        .bind(source_event_ids)
        .bind(text)
        .bind(category)
        .bind(confidence)
        .bind(sensitivity)
        .bind("pending_review")
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .map(|_| ()),
        StorageBackend::Postgres(p) => sqlx::query(
            "INSERT INTO memory_candidate
                    (id, workspace_id, proposed_by_actor_id, source_event_ids,
                     text, category, confidence, sensitivity, status, created_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        )
        .bind(id.as_str())
        .bind(ws.as_str())
        .bind(actor.as_str())
        .bind(source_event_ids)
        .bind(text)
        .bind(category)
        .bind(confidence)
        .bind(sensitivity)
        .bind("pending_review")
        .bind(now_rfc3339())
        .execute(p.pool())
        .await
        .map(|_| ()),
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

type MemoryCandidateRow = (String, String, String, f64, String, String);

async fn memory_candidate(
    backend: &StorageBackend,
    id: &str,
) -> Result<Option<MemoryCandidateRow>, ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => {
            sqlx::query_as::<_, MemoryCandidateRow>(
                "SELECT id, text, category, confidence, sensitivity, source_event_ids
                 FROM memory_candidate WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(s.pool())
            .await
        }
        StorageBackend::Postgres(p) => {
            sqlx::query_as::<_, MemoryCandidateRow>(
                "SELECT id, text, category, confidence, sensitivity, source_event_ids
                 FROM memory_candidate WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(p.pool())
            .await
        }
    }
    .map_err(|e| ActantError::Storage(e.to_string()))
}

#[allow(clippy::too_many_arguments)]
async fn insert_memory(
    backend: &StorageBackend,
    id: &MemoryId,
    ws: &WorkspaceId,
    text: &str,
    category: &str,
    sensitivity: &str,
    confidence: f64,
    candidate_id: &str,
    source_event_ids: &str,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => sqlx::query(
            "INSERT INTO memory
                    (id, workspace_id, text, category, sensitivity, confidence,
                     scope, source_candidate_id, source_event_ids, usage_count, created_at)
                 VALUES (?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(id.as_str())
        .bind(ws.as_str())
        .bind(text)
        .bind(category)
        .bind(sensitivity)
        .bind(confidence)
        .bind("global")
        .bind(candidate_id)
        .bind(source_event_ids)
        .bind(0i64)
        .bind(now_rfc3339())
        .execute(s.pool())
        .await
        .map(|_| ()),
        StorageBackend::Postgres(p) => sqlx::query(
            "INSERT INTO memory
                    (id, workspace_id, text, category, sensitivity, confidence,
                     scope, source_candidate_id, source_event_ids, usage_count, created_at)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
        )
        .bind(id.as_str())
        .bind(ws.as_str())
        .bind(text)
        .bind(category)
        .bind(sensitivity)
        .bind(confidence)
        .bind("global")
        .bind(candidate_id)
        .bind(source_event_ids)
        .bind(0i64)
        .bind(now_rfc3339())
        .execute(p.pool())
        .await
        .map(|_| ()),
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

async fn approve_memory_candidate(backend: &StorageBackend, id: &str) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => {
            sqlx::query("UPDATE memory_candidate SET status='approved' WHERE id=?")
                .bind(id)
                .execute(s.pool())
                .await
                .map(|_| ())
        }
        StorageBackend::Postgres(p) => {
            sqlx::query("UPDATE memory_candidate SET status='approved' WHERE id=$1")
                .bind(id)
                .execute(p.pool())
                .await
                .map(|_| ())
        }
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}

async fn reject_memory_candidate(
    backend: &StorageBackend,
    id: &str,
    reason: &str,
) -> Result<(), ActantError> {
    match backend {
        StorageBackend::Sqlite(s) => {
            sqlx::query("UPDATE memory_candidate SET status='rejected', review_reason=? WHERE id=?")
                .bind(reason)
                .bind(id)
                .execute(s.pool())
                .await
                .map(|_| ())
        }
        StorageBackend::Postgres(p) => sqlx::query(
            "UPDATE memory_candidate SET status='rejected', review_reason=$1 WHERE id=$2",
        )
        .bind(reason)
        .bind(id)
        .execute(p.pool())
        .await
        .map(|_| ()),
    }
    .map_err(|e| ActantError::Storage(e.to_string()))?;
    Ok(())
}
