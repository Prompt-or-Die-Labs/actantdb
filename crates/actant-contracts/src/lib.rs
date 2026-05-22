//! actant-contracts — the single source of truth for ActantDB public types.
//!
//! Every cross-package type lives here. Other ActantDB crates and SDKs
//! consume their types from here; hand-edits to generated outputs are
//! forbidden. See `/CLAUDE.md`.
//!
//! v0.1 scope: only the types the killer demo emits (Guard Authority +
//! Chronicle Replay). Per anti-scope rule #2, nothing here exists without
//! a use site in the demo.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod cache;
pub mod events;
pub mod index;
pub mod models;
pub mod policy;
pub mod replay;
pub mod schema;
pub mod sdk_codegen;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub use cache::*;
pub use events::*;
pub use index::*;
pub use models::*;
pub use policy::*;
pub use replay::*;

const USD_EPSILON: f64 = 0.000_001;
const TRUST_PRIOR_COUNT: f64 = 2.0;

/// One embedding vector and the model that produced it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Embedding {
    /// Provider id.
    pub provider: String,
    /// Model name.
    pub model: String,
    /// Vector.
    pub vector: Vec<f32>,
}

/// Hot-path tool-call request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantHotToolCall {
    /// Workspace identifier.
    pub workspace_id: String,
    /// Calling actor identifier.
    pub actor_id: String,
    /// Session identifier.
    pub session_id: String,
    /// Tool name.
    pub tool: String,
    /// Arguments.
    pub arguments: serde_json::Value,
}

/// A prompt template version.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantPromptVersion {
    /// Version number.
    pub version: u32,
    /// Template body.
    pub body: String,
}

/// A prompt template.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantPromptTemplate {
    /// Display name.
    pub name: String,
    /// Versions, ordered ascending.
    pub versions: Vec<ActantPromptVersion>,
}

impl ActantPromptTemplate {
    /// Latest version.
    pub fn latest(&self) -> Option<&ActantPromptVersion> {
        self.versions.last()
    }

    /// Render a version with `{{var}}` substitutions.
    pub fn render(&self, version: u32, vars: &serde_json::Value) -> Option<String> {
        let v = self.versions.iter().find(|v| v.version == version)?;
        Some(interpolate_prompt(&v.body, vars))
    }
}

fn interpolate_prompt(body: &str, vars: &serde_json::Value) -> String {
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            chars.next();
            let mut key = String::new();
            while let Some(c2) = chars.next() {
                if c2 == '}' && chars.peek() == Some(&'}') {
                    chars.next();
                    break;
                }
                key.push(c2);
            }
            let key = key.trim();
            let val = vars.get(key).and_then(|v| v.as_str()).unwrap_or("");
            out.push_str(val);
        } else {
            out.push(c);
        }
    }
    out
}

/// MCP server descriptor.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantMcpServer {
    /// Display name.
    pub name: String,
    /// Transport ("stdio", "http", "websocket").
    pub transport: String,
    /// URI of the server.
    pub uri: String,
    /// Capabilities advertised.
    pub capabilities: Vec<String>,
}

/// A2A peer card.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantA2aCard {
    /// Peer display name.
    pub peer_name: String,
    /// Endpoint URL.
    pub endpoint: String,
    /// Advertised capabilities.
    pub capabilities: Vec<String>,
}

/// AP2 mandate: delegated authority to spend.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantAp2Mandate {
    /// Grantee.
    pub holder: String,
    /// Purpose.
    pub purpose: String,
    /// Spend limit in USD.
    pub spend_limit_usd: f64,
}

impl ActantAp2Mandate {
    /// Validate a proposed amount.
    pub fn permits(&self, amount_usd: f64) -> bool {
        amount_usd.is_finite()
            && amount_usd >= 0.0
            && self.spend_limit_usd.is_finite()
            && self.spend_limit_usd >= 0.0
            && amount_usd <= self.spend_limit_usd + USD_EPSILON
    }
}

/// Memory storage scope allowed by a capsule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAllowed {
    /// Do not write derived content to memory.
    Forbidden,
    /// Workspace-local memory writes are allowed.
    Workspace,
    /// Global memory writes are allowed.
    Global,
}

/// Policy bundle that travels with derived content.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantCapsule {
    /// Display name.
    pub name: String,
    /// Highest sensitivity content this capsule may hold.
    pub sensitivity: Sensitivity,
    /// Allow this capsule's content to be sent to cloud models.
    pub cloud_model_allowed: bool,
    /// Memory storage scope allowed by this capsule.
    pub memory_allowed: MemoryAllowed,
    /// Sensitivity to upgrade to when re-emitted as memory.
    pub upgrades_to_sensitivity: Option<Sensitivity>,
}

impl ActantCapsule {
    /// Default-deny capsule for unknown content.
    pub fn default_deny(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sensitivity: Sensitivity::High,
            cloud_model_allowed: false,
            memory_allowed: MemoryAllowed::Forbidden,
            upgrades_to_sensitivity: None,
        }
    }

    /// Visible cloud-allowed capsule.
    pub fn public_okay(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            sensitivity: Sensitivity::Low,
            cloud_model_allowed: true,
            memory_allowed: MemoryAllowed::Workspace,
            upgrades_to_sensitivity: None,
        }
    }

    /// Returns true if this capsule permits routing to a cloud model.
    pub fn cloud_allowed(&self) -> bool {
        self.cloud_model_allowed && !matches!(self.sensitivity, Sensitivity::Secret)
    }
}

/// Trust profile for one actor + capability area.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActantTrustProfile {
    /// Capability area.
    pub area: String,
    /// Score in \[0, 1].
    pub score: f64,
    /// Bayesian confidence given sample size.
    pub confidence: f64,
    /// Number of observations.
    pub sample_size: u64,
}

impl ActantTrustProfile {
    /// Cold start.
    pub fn new(area: impl Into<String>) -> Self {
        Self {
            area: area.into(),
            score: 0.5,
            confidence: 0.0,
            sample_size: 0,
        }
    }

    /// Record a success or failure observation.
    pub fn observe(&mut self, success: bool) {
        let outcome = if success { 1.0 } else { 0.0 };
        let n = self.sample_size as f64;
        self.score =
            (self.score * (n + TRUST_PRIOR_COUNT) + outcome) / (n + TRUST_PRIOR_COUNT + 1.0);
        self.sample_size += 1;
        let effective_sample_size = self.sample_size as f64 + TRUST_PRIOR_COUNT;
        self.confidence = effective_sample_size / (effective_sample_size + 10.0);
    }

    /// Returns true when this profile permits auto-approval.
    pub fn auto_approve_at(&self, min_score: f64, min_confidence: f64) -> bool {
        self.score >= min_score && self.confidence >= min_confidence
    }
}
