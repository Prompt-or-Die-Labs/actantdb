//! Browser-effect worker.
//!
//! Four operations:
//!
//!   - `browser.navigate`   — visit a URL, returns the title.
//!   - `browser.click`      — click a CSS selector.
//!   - `browser.type`       — type text into a field.
//!   - `browser.screenshot` — capture the page (PNG byte count + dims).
//!
//! Two driver implementations live behind the `Driver` trait:
//!
//!   - [`EmulatorDriver`] — deterministic in-process recorder, default.
//!   - [`cdp::CdpDriver`] — real headless Chrome via Chrome DevTools
//!     Protocol, gated behind the `cdp` cargo feature.

#[cfg(feature = "cdp")]
pub mod cdp;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use actant_core::ActantError;
use actant_worker_protocol::{Handler, HandlerResult};
use async_trait::async_trait;

/// One recorded browser interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Navigate.
    Navigate(String),
    /// Click selector.
    Click(String),
    /// Type text into selector.
    Type(String, String),
    /// Screenshot.
    Screenshot,
}

/// A browser driver.
#[async_trait]
pub trait Driver: Send + Sync {
    /// Run one effect.
    async fn run(&self, action: Action) -> Result<serde_json::Value, ActantError>;
}

/// Deterministic in-memory driver. Records actions to a queue.
#[derive(Debug, Default, Clone)]
pub struct EmulatorDriver {
    /// Page title to report.
    pub title: String,
    actions: Arc<Mutex<VecDeque<Action>>>,
}

impl EmulatorDriver {
    /// New driver with a default title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            actions: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Drain the recorded actions (for tests).
    pub fn recorded(&self) -> Vec<Action> {
        self.actions.lock().unwrap().iter().cloned().collect()
    }
}

#[async_trait]
impl Driver for EmulatorDriver {
    async fn run(&self, action: Action) -> Result<serde_json::Value, ActantError> {
        let a = action.clone();
        self.actions.lock().unwrap().push_back(a);
        Ok(match action {
            Action::Navigate(_) => serde_json::json!({"title": self.title}),
            Action::Click(sel) => serde_json::json!({"clicked": sel}),
            Action::Type(sel, text) => serde_json::json!({"typed": text, "into": sel}),
            Action::Screenshot => serde_json::json!({"hash": "stub"}),
        })
    }
}

/// Handler for `browser.*` effects.
#[derive(Debug)]
pub struct BrowserHandler<D: Driver> {
    /// Underlying driver.
    pub driver: D,
}

impl<D: Driver + 'static> BrowserHandler<D> {
    /// New handler with the given driver.
    pub fn new(driver: D) -> Self {
        Self { driver }
    }
}

#[async_trait]
impl<D: Driver + 'static> Handler for BrowserHandler<D> {
    fn effect_type(&self) -> &'static str {
        "browser.navigate"
    }
    fn effect_types(&self) -> &'static [&'static str] {
        &[
            "browser.navigate",
            "browser.click",
            "browser.type",
            "browser.screenshot",
        ]
    }

    async fn handle(&self, input: serde_json::Value) -> HandlerResult {
        let kind = input
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("navigate");
        let action = match kind {
            "navigate" => {
                let url = input
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ActantError::InvalidInput("missing url".into()))?
                    .to_string();
                Action::Navigate(url)
            }
            "click" => {
                let sel = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ActantError::InvalidInput("missing selector".into()))?
                    .to_string();
                Action::Click(sel)
            }
            "type" => {
                let sel = input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ActantError::InvalidInput("missing selector".into()))?
                    .to_string();
                let text = input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ActantError::InvalidInput("missing text".into()))?
                    .to_string();
                Action::Type(sel, text)
            }
            "screenshot" => Action::Screenshot,
            other => {
                return Err(ActantError::InvalidInput(format!(
                    "unknown browser op: {other}"
                )))
            }
        };
        self.driver.run(action).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn navigate_and_click_recorded() {
        let driver = EmulatorDriver::new("example.com");
        let h = BrowserHandler::new(driver.clone());
        let r = h
            .handle(serde_json::json!({"op":"navigate","url":"https://example.com"}))
            .await
            .unwrap();
        assert_eq!(r["title"], "example.com");
        h.handle(serde_json::json!({"op":"click","selector":"button#go"}))
            .await
            .unwrap();
        h.handle(serde_json::json!({"op":"type","selector":"#q","text":"hello"}))
            .await
            .unwrap();
        let rec = driver.recorded();
        assert_eq!(rec.len(), 3);
        assert_eq!(rec[0], Action::Navigate("https://example.com".into()));
        assert_eq!(rec[1], Action::Click("button#go".into()));
        assert_eq!(rec[2], Action::Type("#q".into(), "hello".into()));
    }
}
