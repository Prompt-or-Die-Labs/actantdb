//! WebSocket subscription stream for `GET /v1/ws`.

use std::pin::Pin;

use futures::stream::Stream;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::client::ActantClient;
use crate::error::{ActantError, Result};
use crate::types::SubscriptionTopic;

/// One message delivered over a `/v1/ws` subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionMessage {
    /// Topic this message was published to.
    pub topic: SubscriptionTopic,
    /// Free-form payload (command response, event, etc.).
    pub payload: Value,
    /// RFC3339 publish time.
    #[serde(default)]
    pub published_at: Option<String>,
}

/// Topic kinds the server publishes by default. Free-form strings are still
/// accepted via [`crate::ActantClient::subscribe_with_kind`].
#[derive(Debug, Clone, Copy)]
pub enum SubscriptionKind {
    /// Chronicle events (default).
    Events,
    /// Approval lifecycle.
    Approvals,
}

impl SubscriptionKind {
    /// Wire string.
    pub fn as_str(&self) -> &'static str {
        match self {
            SubscriptionKind::Events => "events",
            SubscriptionKind::Approvals => "approvals",
        }
    }
}

/// Boxed stream returned by [`ActantClient::subscribe`].
pub type SubscriptionStream =
    Pin<Box<dyn Stream<Item = Result<SubscriptionMessage>> + Send + 'static>>;

impl ActantClient {
    /// Subscribe to a topic via `/v1/ws`. The returned stream yields one item
    /// per published frame; on graceful close it ends without error. Drop the
    /// stream (or the task driving it) to close the underlying socket.
    ///
    /// Frames that don't parse as a [`SubscriptionMessage`] are surfaced as
    /// [`ActantError::Decoding`] so the caller can log + decide; the stream
    /// itself stays open.
    pub async fn subscribe(
        &self,
        workspace_id: Option<&str>,
        session_id: Option<&str>,
        kind: SubscriptionKind,
    ) -> Result<SubscriptionStream> {
        self.subscribe_with_kind(workspace_id, session_id, kind.as_str())
            .await
    }

    /// Like [`Self::subscribe`] but accepts an arbitrary `kind` string. Useful
    /// for forward-compat with new server topic kinds.
    pub async fn subscribe_with_kind(
        &self,
        workspace_id: Option<&str>,
        session_id: Option<&str>,
        kind: &str,
    ) -> Result<SubscriptionStream> {
        let ws = workspace_id
            .map(str::to_string)
            .or_else(|| self.default_workspace_id().map(str::to_string))
            .ok_or_else(|| ActantError::InvalidInput {
                message: "workspace_id required for subscribe".into(),
                body: Vec::new(),
            })?;

        let mut query: Vec<(&str, &str)> = vec![("workspace_id", ws.as_str()), ("kind", kind)];
        if let Some(s) = session_id {
            query.push(("session_id", s));
        }
        let mut url = self.join_url("/v1/ws", &query)?;
        let scheme: &'static str = match url.scheme() {
            "http" => "ws",
            "https" => "wss",
            "ws" => "ws",
            "wss" => "wss",
            other => {
                return Err(ActantError::InvalidUrl(format!(
                    "unsupported scheme: {other}"
                )))
            }
        };
        url.set_scheme(scheme)
            .map_err(|_| ActantError::InvalidUrl("could not set ws scheme".into()))?;

        let mut req = url
            .as_str()
            .into_client_request()
            .map_err(|e| ActantError::WebSocket(e.to_string()))?;
        if let Some(tok) = self.token_ref() {
            let header_value = HeaderValue::from_str(&format!("Bearer {tok}"))
                .map_err(|e| ActantError::WebSocket(e.to_string()))?;
            req.headers_mut().insert("authorization", header_value);
        }

        let (ws_stream, _resp) = tokio_tungstenite::connect_async(req)
            .await
            .map_err(|e| ActantError::WebSocket(e.to_string()))?;
        let (mut sink, mut source) = ws_stream.split();

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<SubscriptionMessage>>();
        tokio::spawn(async move {
            while let Some(frame) = source.next().await {
                match frame {
                    Ok(Message::Text(t)) => {
                        let item = serde_json::from_str::<SubscriptionMessage>(&t).map_err(|e| {
                            ActantError::Decoding {
                                message: e.to_string(),
                                body: t.as_bytes().to_vec(),
                            }
                        });
                        if tx.send(item).is_err() {
                            break;
                        }
                    }
                    Ok(Message::Binary(b)) => {
                        let item = serde_json::from_slice::<SubscriptionMessage>(&b).map_err(|e| {
                            ActantError::Decoding {
                                message: e.to_string(),
                                body: b.clone(),
                            }
                        });
                        if tx.send(item).is_err() {
                            break;
                        }
                    }
                    Ok(Message::Ping(p)) => {
                        // Best-effort pong. If the sink is gone, drop out.
                        if sink.send(Message::Pong(p)).await.is_err() {
                            break;
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Ok(_) => continue,
                    Err(e) => {
                        let _ = tx.send(Err(ActantError::WebSocket(e.to_string())));
                        break;
                    }
                }
            }
        });
        Ok(Box::pin(
            tokio_stream::wrappers::UnboundedReceiverStream::new(rx),
        ))
    }
}

// Provide a tiny shim so `subscribe.rs` can build without an explicit
// `tokio-stream` dep at the top level — we add it via the same workspace
// tokio feature surface.
mod tokio_stream {
    pub mod wrappers {
        use futures::stream::Stream;
        use std::pin::Pin;
        use std::task::{Context, Poll};
        use tokio::sync::mpsc;

        /// Mirrors `tokio_stream::wrappers::UnboundedReceiverStream` minus
        /// the dependency. The full crate adds 0 features we need beyond
        /// this one wrapper.
        pub struct UnboundedReceiverStream<T> {
            inner: mpsc::UnboundedReceiver<T>,
        }

        impl<T> UnboundedReceiverStream<T> {
            /// Wrap an unbounded receiver as a `Stream`.
            pub fn new(rx: mpsc::UnboundedReceiver<T>) -> Self {
                Self { inner: rx }
            }
        }

        impl<T> Stream for UnboundedReceiverStream<T> {
            type Item = T;
            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                self.inner.poll_recv(cx)
            }
        }
    }
}
