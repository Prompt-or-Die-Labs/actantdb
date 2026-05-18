//! Real Chrome DevTools Protocol driver.
//!
//! Drop-in replacement for [`crate::EmulatorDriver`] that talks to a real
//! Chromium instance via the Chrome DevTools Protocol using
//! [`chromiumoxide`]. Gated behind the `cdp` cargo feature so the default
//! workspace build does not require a Chrome install.
//!
//! ```no_run
//! # #[cfg(feature = "cdp")]
//! # async fn ex() -> anyhow::Result<()> {
//! use actant_worker_browser::{Action, BrowserHandler, Driver, cdp::CdpDriver};
//!
//! let driver = CdpDriver::launch_headless().await?;
//! let _r = driver.run(Action::Navigate("data:text/html,<h1>hi</h1>".into())).await?;
//! driver.close().await?;
//! # Ok(()) }
//! ```

use std::sync::Arc;

use actant_core::ActantError;
use async_trait::async_trait;
use chromiumoxide::browser::{Browser, BrowserConfig, BrowserConfigBuilder};
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::{Action, Driver};

/// Real CDP-backed [`Driver`] implementation.
///
/// One `CdpDriver` owns one `Browser` connection. The handler stream that
/// chromiumoxide returns must be polled to keep the connection alive; we
/// spawn a background task for that and join it in [`Self::close`].
///
/// `CdpDriver` is `Clone` (via `Arc` interior) so it slots into
/// [`crate::BrowserHandler`] the same way [`crate::EmulatorDriver`] does.
#[derive(Clone)]
pub struct CdpDriver {
    inner: Arc<Inner>,
}

struct Inner {
    browser: Mutex<Browser>,
    handler_task: Mutex<Option<JoinHandle<()>>>,
}

impl std::fmt::Debug for CdpDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdpDriver").finish_non_exhaustive()
    }
}

fn cdp_err(e: impl std::fmt::Display) -> ActantError {
    ActantError::Internal(format!("cdp: {e}"))
}

impl CdpDriver {
    /// Launch a headless Chrome/Chromium and connect via CDP.
    pub async fn launch_headless() -> Result<Self, ActantError> {
        Self::launch_with(BrowserConfig::builder()).await
    }

    /// Launch a head-ful (visible window) Chrome/Chromium and connect via CDP.
    pub async fn launch_headed() -> Result<Self, ActantError> {
        Self::launch_with(BrowserConfig::builder().with_head()).await
    }

    /// Launch with a custom [`BrowserConfigBuilder`] (e.g. to set
    /// `--no-sandbox` in containerised CI, or point at a specific Chrome
    /// binary).
    pub async fn launch_with(builder: BrowserConfigBuilder) -> Result<Self, ActantError> {
        let config = builder.build().map_err(cdp_err)?;
        let (browser, mut handler) = Browser::launch(config).await.map_err(cdp_err)?;
        let handler_task = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                if let Err(e) = event {
                    tracing::debug!(error = %e, "cdp handler event error");
                }
            }
        });
        Ok(Self {
            inner: Arc::new(Inner {
                browser: Mutex::new(browser),
                handler_task: Mutex::new(Some(handler_task)),
            }),
        })
    }

    /// Cleanly close the underlying browser and join the handler task.
    /// Safe to call multiple times.
    pub async fn close(&self) -> Result<(), ActantError> {
        {
            let mut browser = self.inner.browser.lock().await;
            // Best-effort close; the handler task will exit once the
            // connection is gone.
            let _ = browser.close().await;
            let _ = browser.wait().await;
        }
        let mut slot = self.inner.handler_task.lock().await;
        if let Some(task) = slot.take() {
            task.abort();
            let _ = task.await;
        }
        Ok(())
    }
}

#[async_trait]
impl Driver for CdpDriver {
    async fn run(&self, action: Action) -> Result<serde_json::Value, ActantError> {
        match action {
            Action::Navigate(url) => {
                let mut browser = self.inner.browser.lock().await;
                let page = browser.new_page(url.as_str()).await.map_err(cdp_err)?;
                let title = page.get_title().await.map_err(cdp_err)?.unwrap_or_default();
                Ok(serde_json::json!({"title": title}))
            }
            Action::Click(sel) => {
                let mut browser = self.inner.browser.lock().await;
                // CDP requires an open page; reuse the most recently-opened
                // tab if one exists, otherwise open about:blank.
                let pages = browser.pages().await.map_err(cdp_err)?;
                let page = match pages.into_iter().last() {
                    Some(p) => p,
                    None => browser.new_page("about:blank").await.map_err(cdp_err)?,
                };
                let element = page.find_element(sel.as_str()).await.map_err(cdp_err)?;
                element.click().await.map_err(cdp_err)?;
                Ok(serde_json::json!({"clicked": sel}))
            }
            Action::Type(sel, text) => {
                let mut browser = self.inner.browser.lock().await;
                let pages = browser.pages().await.map_err(cdp_err)?;
                let page = match pages.into_iter().last() {
                    Some(p) => p,
                    None => browser.new_page("about:blank").await.map_err(cdp_err)?,
                };
                let element = page.find_element(sel.as_str()).await.map_err(cdp_err)?;
                element.click().await.map_err(cdp_err)?;
                element.type_str(text.as_str()).await.map_err(cdp_err)?;
                Ok(serde_json::json!({"typed": text, "into": sel}))
            }
            Action::Screenshot => {
                let mut browser = self.inner.browser.lock().await;
                let pages = browser.pages().await.map_err(cdp_err)?;
                let page = match pages.into_iter().last() {
                    Some(p) => p,
                    None => browser.new_page("about:blank").await.map_err(cdp_err)?,
                };
                let png = page
                    .screenshot(ScreenshotParams::default())
                    .await
                    .map_err(cdp_err)?;
                Ok(serde_json::json!({
                    "bytes": png.len(),
                    "format": "png",
                }))
            }
        }
    }
}
