//! File-effect worker.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::path::{Component, Path, PathBuf};

use actant_core::ActantError;
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;

/// Validate that `candidate` resolves to a path strictly inside `base`.
///
/// Returns `Ok(resolved)` where `resolved = base.join(<sanitized candidate>)`
/// is guaranteed (lexically) to live under `base`. Returns
/// `ActantError::InvalidInput` for any of:
///
/// - candidate contains a NUL byte;
/// - candidate is absolute (would escape `base` entirely);
/// - candidate contains a `..` (parent-dir) component;
/// - candidate normalizes to an empty path.
///
/// Lexical check only — no filesystem access. The `path_fuzz` test then
/// follows up with a real write on every accepted path and asserts the
/// post-canonicalize result is still under `base`.
pub fn validate_path(base: &Path, candidate: &Path) -> Result<PathBuf, ActantError> {
    let raw = candidate.as_os_str().to_string_lossy();
    if raw.as_bytes().contains(&0) {
        return Err(ActantError::InvalidInput("path contains NUL byte".into()));
    }
    if candidate.is_absolute() {
        return Err(ActantError::InvalidInput(format!(
            "absolute paths are rejected: {}",
            candidate.display()
        )));
    }
    let mut normalized = PathBuf::new();
    for c in candidate.components() {
        match c {
            Component::Prefix(_) | Component::RootDir => {
                return Err(ActantError::InvalidInput(format!(
                    "path component escapes root: {}",
                    candidate.display()
                )));
            }
            Component::ParentDir => {
                return Err(ActantError::InvalidInput(format!(
                    "parent-dir traversal rejected: {}",
                    candidate.display()
                )));
            }
            Component::CurDir => {}
            Component::Normal(seg) => normalized.push(seg),
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(ActantError::InvalidInput("empty path".into()));
    }
    Ok(base.join(normalized))
}

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
