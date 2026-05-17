//! actant-schema-dsl — small types-and-parser for ActantDB's project schema DSL.
//!
//! Phase 1: accept a YAML-ish document describing tools, models, and policies.
//! Emit a strongly-typed in-memory representation that other crates consume.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// Top-level project descriptor.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Project {
    /// Project name.
    pub name: String,
    /// Tool declarations.
    #[serde(default)]
    pub tools: Vec<ToolDecl>,
    /// Model route declarations.
    #[serde(default)]
    pub models: Vec<ModelDecl>,
}

/// One tool declaration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDecl {
    /// Tool name (`shell.run`, `file.write`, ...).
    pub name: String,
    /// Kind: shell/file/http/mcp/...
    pub kind: String,
    /// Default risk level.
    #[serde(default = "default_risk")]
    pub risk: String,
}

fn default_risk() -> String {
    "medium".into()
}

/// One model route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDecl {
    /// Route name.
    pub name: String,
    /// Provider.
    pub provider: String,
    /// Model name.
    pub model: String,
}

/// Parse a JSON or YAML-style declaration. Phase 1 accepts JSON only.
pub fn parse_json(s: &str) -> Result<Project, serde_json::Error> {
    serde_json::from_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_project() {
        let p = parse_json(
            r#"{
                "name": "demo",
                "tools": [{"name":"shell.run","kind":"shell","risk":"critical"}],
                "models": [{"name":"planner","provider":"anthropic","model":"sonnet"}]
            }"#,
        )
        .unwrap();
        assert_eq!(p.tools.len(), 1);
        assert_eq!(p.tools[0].risk, "critical");
    }
}
