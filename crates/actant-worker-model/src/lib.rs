//! Model-call worker. Adapts OpenAI-compatible HTTP providers and a mock
//! provider used by tests and the alpha-demo without keys.
//!
//! Phase 1 keeps the surface small: one `model.call` effect type with an
//! `input` schema of `{ prompt, model, route, role }`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::ActantError;
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;

/// Provider for the model worker.
#[derive(Debug, Clone)]
pub enum Provider {
    /// Deterministic mock — echoes the prompt back as a "plan".
    Mock,
    /// OpenAI-compatible chat completion API.
    OpenAi {
        /// Base URL like `https://api.openai.com/v1`.
        base_url: String,
        /// Bearer token.
        api_key: String,
    },
}

/// Handler for `model.call` effects.
#[derive(Debug, Clone)]
pub struct ModelHandler {
    /// Active provider.
    pub provider: Provider,
}

impl ModelHandler {
    /// New mock handler.
    pub fn mock() -> Self {
        Self {
            provider: Provider::Mock,
        }
    }
}

#[async_trait]
impl Handler for ModelHandler {
    fn effect_type(&self) -> &'static str {
        "model.call"
    }

    async fn handle(&self, input: serde_json::Value) -> HandlerResult {
        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let model = input
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("mock")
            .to_string();
        match &self.provider {
            Provider::Mock => Ok(serde_json::json!({
                "model": model,
                "tokens_in": prompt.len() as u32 / 4,
                "tokens_out": (prompt.len() as u32 / 8).max(4),
                "summary": format!("[mock] received: {}", &prompt[..prompt.len().min(80)]),
                "raw": prompt,
            })),
            Provider::OpenAi { base_url, api_key } => {
                let client = reqwest::Client::new();
                let resp = client
                    .post(format!("{base_url}/chat/completions"))
                    .bearer_auth(api_key)
                    .json(&serde_json::json!({
                        "model": model,
                        "messages": [{"role":"user","content": prompt}],
                    }))
                    .send()
                    .await
                    .map_err(|e| ActantError::Internal(format!("openai: {e}")))?;
                let body: serde_json::Value = resp
                    .json()
                    .await
                    .map_err(|e| ActantError::Internal(format!("openai parse: {e}")))?;
                Ok(body)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_responds() {
        let h = ModelHandler::mock();
        let r = h
            .handle(serde_json::json!({"prompt":"hi","model":"mock"}))
            .await
            .unwrap();
        assert_eq!(r["model"], "mock");
    }
}
