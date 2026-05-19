//! Email-effect worker.
//!
//! `email.send` with `{to, subject, body}`. Real SMTP is post-Phase 6;
//! the shipped `RecordingMailer` captures sent messages deterministically.

use std::sync::{Arc, Mutex};

use actant_core::ActantError;
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One outbound email.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mail {
    /// Recipient(s).
    pub to: Vec<String>,
    /// Subject line.
    pub subject: String,
    /// Body.
    pub body: String,
}

/// A mailer.
#[async_trait]
pub trait Mailer: Send + Sync {
    /// Send a message.
    async fn send(&self, mail: Mail) -> Result<(), ActantError>;
}

/// Deterministic recording mailer for tests.
#[derive(Debug, Default, Clone)]
pub struct RecordingMailer {
    sent: Arc<Mutex<Vec<Mail>>>,
}

impl RecordingMailer {
    /// New mailer.
    pub fn new() -> Self {
        Self::default()
    }
    /// Snapshot of sent messages.
    pub fn sent(&self) -> Vec<Mail> {
        self.sent.lock().unwrap().clone()
    }
}

#[async_trait]
impl Mailer for RecordingMailer {
    async fn send(&self, mail: Mail) -> Result<(), ActantError> {
        self.sent.lock().unwrap().push(mail);
        Ok(())
    }
}

/// Handler for `email.send`.
#[derive(Debug)]
pub struct EmailHandler<M: Mailer> {
    /// Mailer.
    pub mailer: M,
}

impl<M: Mailer + 'static> EmailHandler<M> {
    /// New.
    pub fn new(mailer: M) -> Self {
        Self { mailer }
    }
}

#[async_trait]
impl<M: Mailer + 'static> Handler for EmailHandler<M> {
    fn effect_type(&self) -> &'static str {
        "email.send"
    }
    async fn handle(&self, input: serde_json::Value) -> HandlerResult {
        let to: Vec<String> = input
            .get("to")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .ok_or_else(|| ActantError::InvalidInput("missing to".into()))?;
        if to.is_empty() {
            return Err(ActantError::InvalidInput("to is empty".into()));
        }
        let subject = input
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let body = input
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mail = Mail {
            to: to.clone(),
            subject: subject.clone(),
            body,
        };
        self.mailer.send(mail).await?;
        Ok(serde_json::json!({"sent": true, "recipients": to}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_one_message() {
        let m = RecordingMailer::new();
        let h = EmailHandler::new(m.clone());
        let r = h
            .handle(serde_json::json!({
                "to": ["alice@example.com"],
                "subject": "hi",
                "body": "hello world"
            }))
            .await
            .unwrap();
        assert_eq!(r["sent"], true);
        let sent = m.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].to, vec!["alice@example.com".to_string()]);
        assert_eq!(sent[0].subject, "hi");
    }

    #[tokio::test]
    async fn rejects_empty_to() {
        let h = EmailHandler::new(RecordingMailer::new());
        let r = h
            .handle(serde_json::json!({"to": [], "subject": "x"}))
            .await;
        assert!(r.is_err());
    }
}
