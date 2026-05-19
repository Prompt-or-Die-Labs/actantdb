//! MCP / A2A / AP2 protocol surfaces.
//!
//! Phase 1 ships data shapes only — wire clients are a Phase 4 deliverable.
//! See `/specs/16-protocols.md`.

use serde::{Deserialize, Serialize};

/// MCP server descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    /// Display name.
    pub name: String,
    /// Transport ("stdio", "http", "websocket").
    pub transport: String,
    /// URI of the server.
    pub uri: String,
    /// Capabilities advertised (e.g. ["tools","resources","prompts"]).
    pub capabilities: Vec<String>,
}

/// A2A peer card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aCard {
    /// Peer display name.
    pub peer_name: String,
    /// Endpoint URL.
    pub endpoint: String,
    /// Advertised capabilities.
    pub capabilities: Vec<String>,
}

/// AP2 mandate (delegated authority to spend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ap2Mandate {
    /// Grantee (the agent given authority).
    pub holder: String,
    /// Purpose (free-form).
    pub purpose: String,
    /// Spend limit (USD).
    pub spend_limit_usd: f64,
}

impl Ap2Mandate {
    /// Validate a proposed amount.
    pub fn permits(&self, amount_usd: f64) -> bool {
        amount_usd <= self.spend_limit_usd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mandate_blocks_overage() {
        let m = Ap2Mandate {
            holder: "agent_x".into(),
            purpose: "subscription".into(),
            spend_limit_usd: 50.0,
        };
        assert!(m.permits(49.0));
        assert!(!m.permits(51.0));
    }
}
