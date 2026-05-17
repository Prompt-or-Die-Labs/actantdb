//! actant-worker-mcp — bridge to MCP servers.
//!
//! Wires JSON-RPC 2.0 over stdio to a child process implementing the Model
//! Context Protocol. One call per `mcp.call` effect: spawn → `initialize` →
//! `tools/call` → read response → exit. Heavier transport modes (HTTP/SSE)
//! and connection pooling land later — stdio is the canonical wire.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::process::Stdio;
use std::time::Duration;

use actant_core::ActantError;
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// MCP tool descriptor.
#[derive(Debug, Clone)]
pub struct McpTool {
    /// Server name.
    pub server: String,
    /// Tool name within the server.
    pub tool: String,
}

/// JSON-RPC 2.0 request frame.
#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: serde_json::Value,
}

/// JSON-RPC 2.0 response frame.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: Option<String>,
    id: Option<u64>,
    #[serde(default)]
    result: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(default)]
    #[allow(dead_code)]
    data: Option<serde_json::Value>,
}

/// Configuration for a single stdio MCP call.
#[derive(Debug, Clone)]
pub struct McpStdioConfig {
    /// Child program path (e.g. `mcp-server-foo`).
    pub program: String,
    /// Args passed to the child program.
    pub args: Vec<String>,
    /// Total deadline for spawn + initialize + call + read.
    pub timeout: Duration,
}

impl Default for McpStdioConfig {
    fn default() -> Self {
        Self {
            program: "mcp-server".to_string(),
            args: Vec::new(),
            timeout: Duration::from_secs(15),
        }
    }
}

/// Stdio MCP client. Owns a child process for the duration of one call.
pub struct McpStdioClient {
    cfg: McpStdioConfig,
}

impl McpStdioClient {
    /// New client with the given config.
    pub fn new(cfg: McpStdioConfig) -> Self {
        Self { cfg }
    }

    /// Make a single `tools/call` against the configured MCP server.
    /// Sequence: spawn → `initialize` → `tools/call` → read → drop child.
    pub async fn call(
        &self,
        tool: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, ActantError> {
        let fut = self.call_inner(tool, arguments);
        tokio::time::timeout(self.cfg.timeout, fut)
            .await
            .map_err(|_| {
                ActantError::Internal(format!("mcp.call timed out after {:?}", self.cfg.timeout))
            })?
    }

    async fn call_inner(
        &self,
        tool: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, ActantError> {
        let mut child = Command::new(&self.cfg.program)
            .args(&self.cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| ActantError::Internal(format!("spawn {}: {e}", self.cfg.program)))?;

        let mut stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");
        let mut reader = BufReader::new(stdout).lines();

        // initialize.
        send(
            &mut stdin,
            &JsonRpcRequest {
                jsonrpc: "2.0",
                id: 1,
                method: "initialize",
                params: serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "actant-worker-mcp", "version": env!("CARGO_PKG_VERSION") }
                }),
            },
        )
        .await?;
        let _init = read_response(&mut reader, 1).await?;

        // tools/call.
        send(
            &mut stdin,
            &JsonRpcRequest {
                jsonrpc: "2.0",
                id: 2,
                method: "tools/call",
                params: serde_json::json!({ "name": tool, "arguments": arguments }),
            },
        )
        .await?;
        let resp = read_response(&mut reader, 2).await?;

        // Close stdin so the child exits cleanly.
        drop(stdin);
        let _ = child.wait().await;
        Ok(resp)
    }
}

async fn send<W: AsyncWriteExt + Unpin>(
    w: &mut W,
    req: &JsonRpcRequest<'_>,
) -> Result<(), ActantError> {
    let mut line =
        serde_json::to_string(req).map_err(|e| ActantError::Internal(format!("encode: {e}")))?;
    line.push('\n');
    w.write_all(line.as_bytes())
        .await
        .map_err(|e| ActantError::Internal(format!("write stdin: {e}")))?;
    w.flush()
        .await
        .map_err(|e| ActantError::Internal(format!("flush stdin: {e}")))?;
    Ok(())
}

async fn read_response<R: AsyncBufReadExt + Unpin>(
    r: &mut tokio::io::Lines<R>,
    want_id: u64,
) -> Result<serde_json::Value, ActantError> {
    loop {
        let line = r
            .next_line()
            .await
            .map_err(|e| ActantError::Internal(format!("read stdout: {e}")))?
            .ok_or_else(|| {
                ActantError::Internal("mcp child closed stdout before responding".into())
            })?;
        if line.trim().is_empty() {
            continue;
        }
        let frame: JsonRpcResponse = serde_json::from_str(&line)
            .map_err(|e| ActantError::Internal(format!("decode mcp frame {line:?}: {e}")))?;
        if frame.id != Some(want_id) {
            continue;
        }
        if let Some(err) = frame.error {
            return Err(ActantError::Internal(format!(
                "mcp error {}: {}",
                err.code, err.message
            )));
        }
        return Ok(frame.result.unwrap_or(serde_json::Value::Null));
    }
}

/// Handler for `mcp.call` effects. Input shape:
/// `{ "program": "mcp-server-xyz", "args": [...], "tool": "name", "arguments": {...} }`.
#[derive(Debug, Default)]
pub struct McpHandler;

#[async_trait]
impl Handler for McpHandler {
    fn effect_type(&self) -> &'static str {
        "mcp.call"
    }

    async fn handle(&self, input: serde_json::Value) -> HandlerResult {
        let program = input
            .get("program")
            .or_else(|| input.get("server"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ActantError::InvalidInput("missing program/server".into()))?
            .to_string();
        let args: Vec<String> = input
            .get("args")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let tool = input
            .get("tool")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ActantError::InvalidInput("missing tool".into()))?
            .to_string();
        let arguments = input
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let timeout_ms = input
            .get("timeout_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(15_000);

        let client = McpStdioClient::new(McpStdioConfig {
            program: program.clone(),
            args,
            timeout: Duration::from_millis(timeout_ms),
        });
        let result = client.call(&tool, arguments).await?;
        Ok(serde_json::json!({
            "program": program,
            "tool": tool,
            "result": result,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn handler_rejects_missing_program() {
        let h = McpHandler;
        let err = h.handle(serde_json::json!({"tool":"x"})).await.unwrap_err();
        assert!(format!("{err}").contains("program"));
    }

    #[tokio::test]
    async fn handler_rejects_missing_tool() {
        let h = McpHandler;
        let err = h
            .handle(serde_json::json!({"program":"x"}))
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("tool"));
    }
}
