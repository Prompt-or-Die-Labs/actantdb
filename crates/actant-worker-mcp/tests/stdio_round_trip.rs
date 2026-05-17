//! End-to-end stdio round-trip: spawn the python fixture MCP server,
//! initialize, call a tool, assert the echoed result.

use std::time::Duration;

use actant_worker_mcp::{McpStdioClient, McpStdioConfig};

fn fixture_script() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/echo_mcp_server.py");
    p
}

fn python3() -> Option<String> {
    // Prefer python3 over python; treat the test as #[ignore] if neither exist.
    for candidate in ["python3", "python"] {
        if std::process::Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(candidate.to_string());
        }
    }
    None
}

#[tokio::test]
async fn stdio_initialize_then_tools_call() {
    let Some(py) = python3() else {
        eprintln!("skipping: no python3 available");
        return;
    };
    let client = McpStdioClient::new(McpStdioConfig {
        program: py,
        args: vec![fixture_script().to_string_lossy().into_owned()],
        timeout: Duration::from_secs(5),
    });
    let result = client
        .call(
            "search",
            serde_json::json!({"query": "actantdb", "limit": 5}),
        )
        .await
        .expect("stdio MCP call");
    assert_eq!(result["name"], "search");
    assert_eq!(result["arguments"]["query"], "actantdb");
    assert_eq!(result["echo"], true);
}

#[tokio::test]
async fn handler_e2e_through_effect_input() {
    let Some(py) = python3() else {
        eprintln!("skipping: no python3 available");
        return;
    };
    use actant_worker_protocol::Handler;
    let h = actant_worker_mcp::McpHandler;
    let resp = h
        .handle(serde_json::json!({
            "program": py,
            "args": [fixture_script().to_string_lossy().into_owned()],
            "tool": "read_file",
            "arguments": {"path": "/tmp/x"},
            "timeout_ms": 5000
        }))
        .await
        .expect("handler call");
    assert_eq!(resp["tool"], "read_file");
    assert_eq!(resp["result"]["name"], "read_file");
    assert_eq!(resp["result"]["arguments"]["path"], "/tmp/x");
}

#[tokio::test]
async fn missing_program_fails_clearly() {
    let client = McpStdioClient::new(McpStdioConfig {
        program: "/this/does/not/exist".into(),
        args: vec![],
        timeout: Duration::from_secs(2),
    });
    let err = client
        .call("x", serde_json::Value::Null)
        .await
        .expect_err("must fail");
    let msg = format!("{err}");
    assert!(msg.contains("spawn"), "msg: {msg}");
}
