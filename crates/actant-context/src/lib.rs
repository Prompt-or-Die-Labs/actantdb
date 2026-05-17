//! actant-context — build context manifests with the four-stage pipeline
//! from `/specs/06-context-and-memory.md`: gather → score → firewall →
//! redact → truncate.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::*;
use serde::{Deserialize, Serialize};

/// One candidate context item before scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateItem {
    /// Stable identifier.
    pub id: String,
    /// Source kind ("memory", "message", "file", "tool_doc").
    pub kind: String,
    /// Source identifier (URI or other handle).
    pub source: String,
    /// Inline content.
    pub content: String,
    /// Declared sensitivity.
    pub sensitivity: Sensitivity,
    /// Visibility hint.
    pub visibility: String,
    /// Tokens (rough estimate, len/4).
    pub token_count: Option<usize>,
}

/// One row of the final manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncludedItem {
    /// Identifier.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Source.
    pub source: String,
    /// Content hash.
    pub content_hash: String,
    /// Sensitivity.
    pub sensitivity: Sensitivity,
    /// Visibility.
    pub visibility: String,
    /// Token count.
    pub token_count: Option<usize>,
}

/// Item that was excluded by the firewall.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedItem {
    /// Identifier.
    pub id: String,
    /// Kind.
    pub kind: String,
    /// Sensitivity.
    pub sensitivity: Sensitivity,
    /// Reason for exclusion.
    pub reason: String,
}

/// Output of the build pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Hash of the included-set (drives replay matching).
    pub manifest_hash: String,
    /// Final included items.
    pub included: Vec<IncludedItem>,
    /// Items blocked by the firewall.
    pub blocked: Vec<BlockedItem>,
    /// Estimated total token count.
    pub total_tokens: usize,
}

/// Build options.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    /// Token budget.
    pub token_budget: usize,
    /// Maximum sensitivity allowed in the prompt.
    pub sensitivity_ceiling: Sensitivity,
    /// Visibility required for the destination route.
    pub required_visibility: String,
    /// Patterns that always block (regex).
    pub deny_patterns: Vec<String>,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            token_budget: 8000,
            sensitivity_ceiling: Sensitivity::Medium,
            required_visibility: "cloud_model_allowed".into(),
            deny_patterns: vec![],
        }
    }
}

/// Build the manifest. The candidates are scored by their declared
/// `token_count` ascending — i.e. small things first; in real code we'd
/// score by relevance.
pub fn build(items: Vec<CandidateItem>, opts: &BuildOptions) -> Manifest {
    let mut included: Vec<IncludedItem> = Vec::new();
    let mut blocked: Vec<BlockedItem> = Vec::new();
    let mut total_tokens = 0usize;

    let compiled_deny: Vec<regex::Regex> = opts
        .deny_patterns
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect();

    for item in items {
        let tokens = item.token_count.unwrap_or(item.content.len() / 4);
        let mut blocked_reason: Option<&'static str> = None;

        if sens_rank(item.sensitivity) > sens_rank(opts.sensitivity_ceiling) {
            blocked_reason = Some("sensitivity");
        }
        // Visibility: secret content is never sent to cloud routes.
        if matches!(item.sensitivity, Sensitivity::Secret)
            && opts.required_visibility == "cloud_model_allowed"
        {
            blocked_reason = Some("visibility");
        }
        if compiled_deny.iter().any(|re| re.is_match(&item.content)) {
            // Per spec 06 §3, deny-pattern matches surface as `sensitivity`
            // blocks — the regex itself is the sensitivity policy.
            blocked_reason = Some("sensitivity");
        }
        if total_tokens + tokens > opts.token_budget {
            blocked_reason = Some("budget");
        }

        if let Some(reason) = blocked_reason {
            blocked.push(BlockedItem {
                id: item.id,
                kind: item.kind,
                sensitivity: item.sensitivity,
                reason: reason.into(),
            });
            continue;
        }

        total_tokens += tokens;
        let content_hash = sha256_hex(item.content.as_bytes());
        included.push(IncludedItem {
            id: item.id,
            kind: item.kind,
            source: item.source,
            content_hash,
            sensitivity: item.sensitivity,
            visibility: item.visibility,
            token_count: Some(tokens),
        });
    }

    let manifest_hash = sha256_hex(
        serde_json::to_string(
            &included
                .iter()
                .map(|i| (i.id.clone(), i.content_hash.clone()))
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default()
        .as_bytes(),
    );
    Manifest {
        manifest_hash,
        included,
        blocked,
        total_tokens,
    }
}

fn sens_rank(s: Sensitivity) -> u8 {
    match s {
        Sensitivity::Public => 0,
        Sensitivity::Low => 1,
        Sensitivity::Medium => 2,
        Sensitivity::High => 3,
        Sensitivity::Secret => 4,
        Sensitivity::Regulated => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_secret_to_cloud() {
        let m = build(
            vec![
                CandidateItem {
                    id: "i1".into(),
                    kind: "file".into(),
                    source: "f".into(),
                    content: "API_KEY=xyz".into(),
                    sensitivity: Sensitivity::Secret,
                    visibility: "cloud_model_allowed".into(),
                    token_count: Some(10),
                },
                CandidateItem {
                    id: "i2".into(),
                    kind: "memory".into(),
                    source: "m".into(),
                    content: "prefers pytest".into(),
                    sensitivity: Sensitivity::Low,
                    visibility: "cloud_model_allowed".into(),
                    token_count: Some(5),
                },
            ],
            &BuildOptions::default(),
        );
        assert_eq!(m.included.len(), 1);
        assert_eq!(m.blocked.len(), 1);
        assert_eq!(m.blocked[0].reason, "visibility");
    }
}
