//! Slack-effect worker.
//!
//! `slack.post` with `{webhook_url, text, channel?}`. The shipped
//! `RecordingPoster` captures posted messages deterministically.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::sync::{Arc, Mutex};

use actant_core::ActantError;
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One Slack post.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Post {
    /// Webhook URL.
    pub webhook_url: String,
    /// Message text.
    pub text: String,
    /// Optional channel override.
    pub channel: Option<String>,
}

/// Poster trait.
#[async_trait]
pub trait Poster: Send + Sync {
    /// Send one post.
    async fn post(&self, post: Post) -> Result<(), ActantError>;
}

/// Deterministic recording poster for tests.
#[derive(Debug, Default, Clone)]
pub struct RecordingPoster {
    sent: Arc<Mutex<Vec<Post>>>,
}

impl RecordingPoster {
    /// New poster.
    pub fn new() -> Self {
        Self::default()
    }
    /// Snapshot of posts.
    pub fn sent(&self) -> Vec<Post> {
        self.sent.lock().unwrap().clone()
    }
}

#[async_trait]
impl Poster for RecordingPoster {
    async fn post(&self, post: Post) -> Result<(), ActantError> {
        self.sent.lock().unwrap().push(post);
        Ok(())
    }
}

/// Real HTTP poster (POSTs JSON to the webhook URL).
#[derive(Debug, Default)]
pub struct HttpPoster {
    client: reqwest::Client,
}

#[async_trait]
impl Poster for HttpPoster {
    async fn post(&self, post: Post) -> Result<(), ActantError> {
        let body = serde_json::json!({
            "text": post.text,
            "channel": post.channel,
        });
        self.client
            .post(&post.webhook_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ActantError::Internal(format!("slack post: {e}")))?;
        Ok(())
    }
}

/// Handler for `slack.post`.
#[derive(Debug)]
pub struct SlackHandler<P: Poster> {
    /// Poster.
    pub poster: P,
}

impl<P: Poster + 'static> SlackHandler<P> {
    /// New.
    pub fn new(poster: P) -> Self {
        Self { poster }
    }
}

#[async_trait]
impl<P: Poster + 'static> Handler for SlackHandler<P> {
    fn effect_type(&self) -> &'static str {
        "slack.post"
    }
    async fn handle(&self, input: serde_json::Value) -> HandlerResult {
        let webhook_url = input
            .get("webhook_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ActantError::InvalidInput("missing webhook_url".into()))?
            .to_string();
        let text = input
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let channel = input
            .get("channel")
            .and_then(|v| v.as_str())
            .map(String::from);
        let post = Post {
            webhook_url,
            text,
            channel,
        };
        self.poster.post(post).await?;
        Ok(serde_json::json!({"posted": true}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_a_post() {
        let p = RecordingPoster::new();
        let h = SlackHandler::new(p.clone());
        h.handle(serde_json::json!({
            "webhook_url": "https://hooks.slack.com/services/x",
            "text": "build green",
            "channel": "#dev"
        }))
        .await
        .unwrap();
        let sent = p.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].text, "build green");
        assert_eq!(sent[0].channel.as_deref(), Some("#dev"));
    }

    #[tokio::test]
    async fn requires_webhook_url() {
        let h = SlackHandler::new(RecordingPoster::new());
        let r = h.handle(serde_json::json!({"text":"x"})).await;
        assert!(r.is_err());
    }
}
