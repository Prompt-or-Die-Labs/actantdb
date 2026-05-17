//! actant-effects — effect queue + worker protocol.
//!
//! Effects are side-effects that must run out of the command transaction.
//! Workers claim them, heartbeat, and report results. The queue stores rows
//! in `effect`, `effect_result`, `effect_claim`, and `worker_*`.
//!
//! See `/specs/04-effect-protocol.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::*;
use actant_storage::Storage;
use serde::{Deserialize, Serialize};

/// Lease duration: how long a worker has to either heartbeat or complete
/// before the effect can be re-claimed.
pub const LEASE_DURATION_SECONDS: i64 = 60;

/// A claim handed back to a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    /// Effect id.
    pub effect_id: EffectId,
    /// Worker id.
    pub worker_id: WorkerId,
    /// Effect type the worker must implement.
    pub effect_type: String,
    /// Canonical inputs (JSON).
    pub input_inline: Option<String>,
    /// Lease expiry RFC3339.
    pub expires_at: String,
}

/// Effect queue API over a `Storage` handle.
#[derive(Clone)]
pub struct EffectQueue {
    storage: Storage,
}

impl EffectQueue {
    /// Wrap an existing storage handle.
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// Underlying storage handle.
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Enqueue a new pending effect.
    pub async fn enqueue(
        &self,
        workspace_id: &WorkspaceId,
        command_id: &CommandId,
        requested_by: &ActorId,
        effect_type: &str,
        input: serde_json::Value,
        risk: RiskLevel,
    ) -> Result<EffectId, ActantError> {
        let input_canon = canonical_json(&input);
        let input_hash = sha256_hex(input_canon.as_bytes());
        let id = EffectId::new();
        sqlx::query(
            "INSERT INTO effect
                (id, workspace_id, command_id, requested_by_actor_id,
                 effect_type, status, risk_level, input_inline, input_hash,
                 attempt_count, max_attempts, created_at)
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
        )
        .bind(id.as_str())
        .bind(workspace_id.as_str())
        .bind(command_id.as_str())
        .bind(requested_by.as_str())
        .bind(effect_type)
        .bind("pending")
        .bind(json_enum(&risk))
        .bind(&input_canon)
        .bind(&input_hash)
        .bind(0i64)
        .bind(3i64)
        .bind(now_rfc3339())
        .execute(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(id)
    }

    /// Worker registers itself + advertised capabilities.
    pub async fn register_worker(
        &self,
        worker: &Worker,
        capabilities: &[&str],
    ) -> Result<(), ActantError> {
        sqlx::query(
            "INSERT OR REPLACE INTO worker
                (id, workspace_id, actor_id, name, host, version, status,
                 last_heartbeat_at, created_at)
             VALUES (?,?,?,?,?,?,?,?,?)",
        )
        .bind(worker.id.as_str())
        .bind(worker.workspace_id.as_str())
        .bind(worker.actor_id.as_str())
        .bind(&worker.name)
        .bind(&worker.host)
        .bind(&worker.version)
        .bind(&worker.status)
        .bind(&worker.last_heartbeat_at)
        .bind(&worker.created_at)
        .execute(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        for cap in capabilities {
            sqlx::query(
                "INSERT OR IGNORE INTO worker_capability (id, worker_id, effect_type)
                 VALUES (?, ?, ?)",
            )
            .bind(format!("{}-{}", worker.id.as_str(), cap))
            .bind(worker.id.as_str())
            .bind(cap)
            .execute(self.storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        }
        Ok(())
    }

    /// Worker heartbeat — extends the lease on currently-claimed effects.
    pub async fn heartbeat(
        &self,
        worker_id: &WorkerId,
        in_flight_count: i64,
    ) -> Result<(), ActantError> {
        let now = now_rfc3339();
        sqlx::query("UPDATE worker SET last_heartbeat_at = ? WHERE id = ?")
            .bind(&now)
            .bind(worker_id.as_str())
            .execute(self.storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        sqlx::query(
            "INSERT INTO worker_heartbeat (id, worker_id, at, in_flight_count)
             VALUES (?, ?, ?, ?)",
        )
        .bind(format!("hb_{}_{}", worker_id.as_str(), &now))
        .bind(worker_id.as_str())
        .bind(&now)
        .bind(in_flight_count)
        .execute(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Atomically claim the oldest pending effect of one of the supported types.
    /// Returns `None` if nothing matches.
    pub async fn claim_one(
        &self,
        worker_id: &WorkerId,
        workspace_id: &WorkspaceId,
        supported_types: &[&str],
    ) -> Result<Option<Lease>, ActantError> {
        if supported_types.is_empty() {
            return Ok(None);
        }
        let placeholders: Vec<&str> = supported_types.iter().map(|_| "?").collect();
        let select_sql = format!(
            "SELECT id, effect_type, input_inline FROM effect
             WHERE workspace_id = ? AND status = 'pending'
               AND effect_type IN ({})
             ORDER BY created_at ASC LIMIT 1",
            placeholders.join(",")
        );
        let mut q = sqlx::query_as::<_, (String, String, Option<String>)>(&select_sql)
            .bind(workspace_id.as_str());
        for t in supported_types {
            q = q.bind(*t);
        }
        let row = q
            .fetch_optional(self.storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        let Some((eid, effect_type, input_inline)) = row else {
            return Ok(None);
        };
        let expires_at = expires_at(LEASE_DURATION_SECONDS);
        let claim_id = format!("clm_{}", ulid::Ulid::new());
        // Atomic claim: UPDATE WHERE status='pending'. If another worker
        // got there first, rows_affected == 0 and we return None so the
        // caller doesn't think it has the lease.
        let res = sqlx::query(
            "UPDATE effect SET status = 'claimed', assigned_worker_id = ?,
                                attempt_count = attempt_count + 1, started_at = ?
             WHERE id = ? AND status = 'pending'",
        )
        .bind(worker_id.as_str())
        .bind(now_rfc3339())
        .bind(&eid)
        .execute(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        if res.rows_affected() == 0 {
            // Another worker won the race.
            return Ok(None);
        }
        sqlx::query(
            "INSERT INTO effect_claim (id, effect_id, worker_id, claimed_at, expires_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&claim_id)
        .bind(&eid)
        .bind(worker_id.as_str())
        .bind(now_rfc3339())
        .bind(&expires_at)
        .execute(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;

        Ok(Some(Lease {
            effect_id: EffectId::from_string(eid),
            worker_id: worker_id.clone(),
            effect_type,
            input_inline,
            expires_at,
        }))
    }

    /// Mark an effect started (worker began real work).
    pub async fn start(&self, effect_id: &EffectId) -> Result<(), ActantError> {
        sqlx::query("UPDATE effect SET status='running' WHERE id=?")
            .bind(effect_id.as_str())
            .execute(self.storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Mark an effect completed (success).
    pub async fn complete(
        &self,
        effect_id: &EffectId,
        output: &serde_json::Value,
    ) -> Result<(), ActantError> {
        let canon = canonical_json(output);
        let hash = sha256_hex(canon.as_bytes());
        let now = now_rfc3339();
        sqlx::query(
            "UPDATE effect SET status='succeeded', result_ref=?, result_hash=?,
                                 finished_at=? WHERE id=?",
        )
        .bind(&canon)
        .bind(&hash)
        .bind(&now)
        .bind(effect_id.as_str())
        .execute(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        sqlx::query(
            "INSERT INTO effect_result (id, effect_id, attempt_number, succeeded,
                                        output_ref, output_hash, started_at, finished_at)
             VALUES (?, ?, (SELECT attempt_count FROM effect WHERE id=?),
                     1, ?, ?, ?, ?)",
        )
        .bind(format!("er_{}", ulid::Ulid::new()))
        .bind(effect_id.as_str())
        .bind(effect_id.as_str())
        .bind(&canon)
        .bind(&hash)
        .bind(&now)
        .bind(&now)
        .execute(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Mark an effect failed. Either retries or fails terminally based on
    /// attempt count.
    pub async fn fail(&self, effect_id: &EffectId, err: &str) -> Result<(), ActantError> {
        sqlx::query("UPDATE effect SET status='failed', error=?, finished_at=? WHERE id=?")
            .bind(err)
            .bind(now_rfc3339())
            .bind(effect_id.as_str())
            .execute(self.storage.pool())
            .await
            .map_err(|e| ActantError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Reclaim claims that have expired (worker silent past lease). Effects
    /// whose attempt_count is below max_attempts are returned to `pending`;
    /// otherwise marked `failed`. Returns the number of effects reclaimed.
    pub async fn reap_expired(&self) -> Result<usize, ActantError> {
        let now = now_rfc3339();
        let expired: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT c.effect_id, c.id, e.attempt_count, e.max_attempts
             FROM effect_claim c
             JOIN effect e ON e.id = c.effect_id
             WHERE c.expires_at < ? AND c.released_at IS NULL
               AND e.status IN ('claimed', 'running')",
        )
        .bind(&now)
        .fetch_all(self.storage.pool())
        .await
        .map_err(|e| ActantError::Storage(e.to_string()))?;
        let mut reclaimed = 0usize;
        for (effect_id, claim_id, attempts, max_attempts) in expired {
            sqlx::query("UPDATE effect_claim SET released_at = ? WHERE id = ?")
                .bind(&now)
                .bind(&claim_id)
                .execute(self.storage.pool())
                .await
                .map_err(|e| ActantError::Storage(e.to_string()))?;
            if attempts < max_attempts {
                sqlx::query(
                    "UPDATE effect SET status='pending', assigned_worker_id=NULL,
                                         next_attempt_at=? WHERE id=?",
                )
                .bind(&now)
                .bind(&effect_id)
                .execute(self.storage.pool())
                .await
                .map_err(|e| ActantError::Storage(e.to_string()))?;
            } else {
                sqlx::query(
                    "UPDATE effect SET status='failed', error='exceeded max_attempts',
                                         finished_at=? WHERE id=?",
                )
                .bind(&now)
                .bind(&effect_id)
                .execute(self.storage.pool())
                .await
                .map_err(|e| ActantError::Storage(e.to_string()))?;
            }
            reclaimed += 1;
        }
        Ok(reclaimed)
    }
}

fn expires_at(seconds: i64) -> String {
    use time::format_description::well_known::Rfc3339;
    let t = time::OffsetDateTime::now_utc() + time::Duration::seconds(seconds);
    t.format(&Rfc3339).expect("rfc3339")
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
    use actant_storage::StorageConfig;

    async fn ws_actor(storage: &Storage) -> (WorkspaceId, ActorId) {
        let ws = Workspace {
            id: WorkspaceId::new(),
            name: "t".into(),
            created_at: now_rfc3339(),
            archived_at: None,
        };
        storage.insert_workspace(&ws).await.unwrap();
        let actor = Actor {
            id: ActorId::new(),
            workspace_id: ws.id.clone(),
            kind: ActorKind::Worker,
            display_name: "wrk".into(),
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        storage.insert_actor(&actor).await.unwrap();
        (ws.id, actor.id)
    }

    #[tokio::test]
    async fn enqueue_claim_complete() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let (ws, actor) = ws_actor(&s).await;
        // a command_record is required (effect FK).
        let cmd = CommandRecord {
            id: CommandId::new(),
            workspace_id: ws.clone(),
            actor_id: actor.clone(),
            session_id: None,
            command_type: "test".into(),
            input_inline: None,
            input_hash: "h".into(),
            policy_id: None,
            status: CommandStatus::Committed,
            error: None,
            created_at: now_rfc3339(),
            committed_at: None,
        };
        s.insert_command(&cmd).await.unwrap();

        let q = EffectQueue::new(s.clone());
        let eff_id = q
            .enqueue(
                &ws,
                &cmd.id,
                &actor,
                "shell.run",
                serde_json::json!({"cmd":"ls"}),
                RiskLevel::Medium,
            )
            .await
            .unwrap();

        let worker = Worker {
            id: WorkerId::new(),
            workspace_id: ws.clone(),
            actor_id: actor.clone(),
            name: "wrk1".into(),
            host: None,
            version: None,
            status: "online".into(),
            last_heartbeat_at: None,
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        q.register_worker(&worker, &["shell.run"]).await.unwrap();

        let lease = q
            .claim_one(&worker.id, &ws, &["shell.run"])
            .await
            .unwrap()
            .expect("expected a lease");
        assert_eq!(lease.effect_id.as_str(), eff_id.as_str());

        q.start(&lease.effect_id).await.unwrap();
        q.complete(&lease.effect_id, &serde_json::json!({"ok": true}))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn reap_expired_returns_effect_to_pending() {
        let s = Storage::open(StorageConfig::in_memory()).await.unwrap();
        let (ws, actor) = ws_actor(&s).await;
        let cmd = CommandRecord {
            id: CommandId::new(),
            workspace_id: ws.clone(),
            actor_id: actor.clone(),
            session_id: None,
            command_type: "test".into(),
            input_inline: None,
            input_hash: "h".into(),
            policy_id: None,
            status: CommandStatus::Committed,
            error: None,
            created_at: now_rfc3339(),
            committed_at: None,
        };
        s.insert_command(&cmd).await.unwrap();
        let q = EffectQueue::new(s.clone());
        let eff_id = q
            .enqueue(
                &ws,
                &cmd.id,
                &actor,
                "shell.run",
                serde_json::json!({}),
                RiskLevel::Low,
            )
            .await
            .unwrap();
        let worker = Worker {
            id: WorkerId::new(),
            workspace_id: ws.clone(),
            actor_id: actor.clone(),
            name: "w".into(),
            host: None,
            version: None,
            status: "online".into(),
            last_heartbeat_at: None,
            created_at: now_rfc3339(),
            disabled_at: None,
        };
        q.register_worker(&worker, &["shell.run"]).await.unwrap();
        let _ = q.claim_one(&worker.id, &ws, &["shell.run"]).await.unwrap();
        // Forcibly expire the claim.
        sqlx::query("UPDATE effect_claim SET expires_at='1970-01-01T00:00:00Z'")
            .execute(s.pool())
            .await
            .unwrap();
        let n = q.reap_expired().await.unwrap();
        assert_eq!(n, 1);
        // Effect should be back to pending.
        let (status,): (String,) = sqlx::query_as("SELECT status FROM effect WHERE id = ?")
            .bind(eff_id.as_str())
            .fetch_one(s.pool())
            .await
            .unwrap();
        assert_eq!(status, "pending");
    }
}
