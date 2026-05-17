//! Shell-effect worker. Executes `shell.run` by spawning a child process.
//!
//! Phase 1 sandboxing is minimal: process is spawned with the worker's own
//! UID; commands run with the host shell. Real OS-level sandboxing is
//! tracked in `/specs/05-security-model.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::ActantError;
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;

/// Handler for `shell.run` effects.
#[derive(Debug, Default)]
pub struct ShellHandler;

#[async_trait]
impl Handler for ShellHandler {
    fn effect_type(&self) -> &'static str {
        "shell.run"
    }

    async fn handle(&self, input: serde_json::Value) -> HandlerResult {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ActantError::InvalidInput("missing command".into()))?
            .to_string();
        let out = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .await
            .map_err(|e| ActantError::Internal(format!("spawn: {e}")))?;
        Ok(serde_json::json!({
            "exit": out.status.code().unwrap_or(-1),
            "stdout": String::from_utf8_lossy(&out.stdout).into_owned(),
            "stderr": String::from_utf8_lossy(&out.stderr).into_owned(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echo_runs() {
        let r = ShellHandler
            .handle(serde_json::json!({"command":"echo hello"}))
            .await
            .unwrap();
        assert_eq!(r["exit"], 0);
        assert!(r["stdout"].as_str().unwrap().contains("hello"));
    }
}
