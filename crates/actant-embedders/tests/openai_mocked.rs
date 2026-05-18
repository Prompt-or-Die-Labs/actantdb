//! Mocked OpenAI embeddings round-trip. Hand-rolled HTTP responder (no
//! `wiremock` workspace dep).
//!
//! Gated behind `--features openai`. The default CI build never compiles
//! this test, since `reqwest` is an opt-in feature dep.

#![cfg(feature = "openai")]

use actant_embed::Embedder;
use actant_embedders::providers::openai::OpenAiEmbedder;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

#[tokio::test]
async fn embed_posts_to_v1_embeddings_and_returns_vector() {
    // Bind a port; respond to one HTTP request, capture the request body for
    // assertions.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let captured: Arc<Mutex<Option<(String, String)>>> = Arc::new(Mutex::new(None));
    let cap_clone = Arc::clone(&captured);

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 4096];
        let mut total = 0;
        let mut header_end = None;
        // Read until end-of-headers.
        loop {
            let n = socket.read(&mut buf[total..]).await.unwrap();
            if n == 0 {
                break;
            }
            total += n;
            if let Some(pos) = find_double_crlf(&buf[..total]) {
                header_end = Some(pos);
                break;
            }
            if total == buf.len() {
                buf.resize(buf.len() * 2, 0);
            }
        }
        let header_end = header_end.expect("client must send full headers");
        let header_str = String::from_utf8_lossy(&buf[..header_end]).to_string();

        // Parse Content-Length.
        let content_length = header_str
            .lines()
            .find_map(|l| {
                let lower = l.to_ascii_lowercase();
                lower
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
            })
            .unwrap_or(0);

        // Read body (some may already be in `buf`).
        let body_start = header_end + 4;
        let mut body = buf[body_start..total].to_vec();
        while body.len() < content_length {
            let mut chunk = vec![0u8; content_length - body.len()];
            let n = socket.read(&mut chunk).await.unwrap();
            if n == 0 {
                break;
            }
            chunk.truncate(n);
            body.extend_from_slice(&chunk);
        }
        let body_str = String::from_utf8_lossy(&body).to_string();

        // Capture for assertions.
        *cap_clone.lock().await = Some((header_str.clone(), body_str));

        // Respond with a tiny fixture.
        let body_resp = r#"{"data":[{"embedding":[0.1,0.2,0.3,0.4]}]}"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body_resp.len(),
            body_resp
        );
        socket.write_all(resp.as_bytes()).await.unwrap();
        socket.shutdown().await.ok();
    });

    let base_url = format!("http://{addr}");
    let client = OpenAiEmbedder::new(base_url, "sk-test-key", "text-embedding-3-small", 4);
    let out = client.embed("hi").await.unwrap();

    server.await.unwrap();

    // Assert response was deserialized correctly.
    assert_eq!(out.vector, vec![0.1f32, 0.2, 0.3, 0.4]);
    assert_eq!(out.provider, "openai");
    assert_eq!(out.model, "text-embedding-3-small");

    // Assert request shape.
    let (headers, body) = captured.lock().await.clone().expect("request captured");
    let first_line = headers.lines().next().unwrap();
    assert!(
        first_line.starts_with("POST /v1/embeddings "),
        "expected POST /v1/embeddings, got: {first_line}"
    );
    let lower = headers.to_ascii_lowercase();
    assert!(
        lower.contains("authorization: bearer sk-test-key"),
        "missing bearer auth header in:\n{headers}"
    );
    assert!(body.contains("\"input\":\"hi\""), "body was: {body}");
    assert!(
        body.contains("\"model\":\"text-embedding-3-small\""),
        "body was: {body}"
    );
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}
