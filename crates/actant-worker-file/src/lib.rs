//! File-effect worker.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::path::PathBuf;

use actant_core::ActantError;
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;

/// Handler for `file.read` and `file.write`.
#[derive(Debug, Default)]
pub struct FileHandler;

#[async_trait]
impl Handler for FileHandler {
    fn effect_type(&self) -> &'static str {
        "file.read"
    }
    fn effect_types(&self) -> &'static [&'static str] {
        &["file.read", "file.write"]
    }

    async fn handle(&self, input: serde_json::Value) -> HandlerResult {
        let path: PathBuf = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ActantError::InvalidInput("missing path".into()))?
            .into();
        if let Some(contents) = input.get("contents").and_then(|v| v.as_str()) {
            tokio::fs::write(&path, contents)
                .await
                .map_err(|e| ActantError::Internal(format!("write: {e}")))?;
            Ok(serde_json::json!({"written": true, "path": path.display().to_string()}))
        } else {
            let body = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| ActantError::Internal(format!("read: {e}")))?;
            Ok(serde_json::json!({"path": path.display().to_string(), "contents": body}))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_then_write() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.txt");
        FileHandler
            .handle(serde_json::json!({"path": p.display().to_string(), "contents": "hi"}))
            .await
            .unwrap();
        let r = FileHandler
            .handle(serde_json::json!({"path": p.display().to_string()}))
            .await
            .unwrap();
        assert_eq!(r["contents"], "hi");
    }
}
