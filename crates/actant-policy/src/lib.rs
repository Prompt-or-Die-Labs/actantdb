//! actant-policy — Guard verdicts for the v2 substrate.
//!
//! Evaluates whether a proposed action (typically a tool call) is allowed
//! under the policy active in a workspace. Verdicts mirror the wedge's
//! [`actant_contracts::PolicyVerdict`] but live on the Rust substrate side.
//!
//! See `/specs/05-security-model.md`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use actant_core::{ActorId, RiskLevel, Sensitivity};
use serde::{Deserialize, Serialize};

/// A Guard verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Verdict {
    /// Permit as-is.
    Allow {
        /// Human-readable reason.
        reason: String,
    },
    /// Permit a rewritten variant.
    Constrain {
        /// Reason.
        reason: String,
        /// Rewritten arguments JSON.
        constrained_input: String,
        /// Human hint.
        hint: String,
    },
    /// Require an approver's decision.
    RequireApproval {
        /// Reason.
        reason: String,
        /// Optional constrain hint.
        hint: Option<String>,
        /// Optional constrained input JSON.
        constrained_input: Option<String>,
    },
    /// Block this action but the run continues.
    Block {
        /// Reason.
        reason: String,
    },
    /// Halt the entire run.
    Halt {
        /// Reason.
        reason: String,
    },
}

impl Verdict {
    /// Stable kind string for ledger payloads.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Allow { .. } => "allow",
            Self::Constrain { .. } => "constrain",
            Self::RequireApproval { .. } => "require_approval",
            Self::Block { .. } => "block",
            Self::Halt { .. } => "halt",
        }
    }
}

/// Policy document.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyDoc {
    /// Display name.
    pub name: String,
    /// Per-tool risk classification.
    pub tools: Vec<ToolPolicy>,
    /// Argument deny rules.
    pub deny: Vec<DenyRule>,
    /// Highest sensitivity allowed without approval.
    pub sensitivity_ceiling: Option<Sensitivity>,
}

/// Per-tool policy entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicy {
    /// Tool name.
    pub tool: String,
    /// Risk class.
    pub risk_level: RiskLevel,
    /// Always require approval for this tool.
    #[serde(default)]
    pub require_approval: bool,
}

/// Regex deny rule applied to a tool's arguments JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenyRule {
    /// Tool this rule applies to. "*" matches any tool.
    pub tool: String,
    /// Regex evaluated against the JSON-stringified args.
    pub pattern: String,
    /// Human reason.
    pub reason: String,
}

/// Tool-call input to Guard's evaluator.
#[derive(Debug, Clone)]
pub struct GuardInput<'a> {
    /// Actor requesting.
    pub actor_id: &'a ActorId,
    /// Tool name.
    pub tool: &'a str,
    /// Arguments as JSON.
    pub arguments_json: &'a str,
    /// Inferred or declared risk.
    pub risk_level: RiskLevel,
    /// Inferred sensitivity of the input.
    pub sensitivity: Sensitivity,
}

/// Evaluate a tool call request against the policy.
pub fn evaluate(policy: &PolicyDoc, input: &GuardInput<'_>) -> Verdict {
    for rule in &policy.deny {
        if rule.tool != "*" && rule.tool != input.tool {
            continue;
        }
        let re = match regex::Regex::new(&rule.pattern) {
            Ok(re) => re,
            Err(_) => continue,
        };
        if re.is_match(input.arguments_json) {
            if let Some(c) = suggest_constrain(input) {
                return Verdict::RequireApproval {
                    reason: rule.reason.clone(),
                    hint: Some(format!("drop {}", c.dropped)),
                    constrained_input: Some(c.args_json),
                };
            }
            return Verdict::Block {
                reason: rule.reason.clone(),
            };
        }
    }

    if let Some(ceiling) = policy.sensitivity_ceiling {
        if sens_rank(input.sensitivity) > sens_rank(ceiling) {
            return Verdict::RequireApproval {
                reason: format!(
                    "args sensitivity {:?} exceeds ceiling {:?}",
                    input.sensitivity, ceiling
                ),
                hint: None,
                constrained_input: None,
            };
        }
    }

    if let Some(entry) = policy.tools.iter().find(|t| t.tool == input.tool) {
        if entry.require_approval {
            return Verdict::RequireApproval {
                reason: format!("tool {} requires approval by policy", input.tool),
                hint: None,
                constrained_input: None,
            };
        }
    }

    if input.tool == "shell.run" {
        if let Some(c) = suggest_constrain(input) {
            return Verdict::RequireApproval {
                reason: "shell.run with destructive pattern".into(),
                hint: Some(format!("drop {}", c.dropped)),
                constrained_input: Some(c.args_json),
            };
        }
        return Verdict::RequireApproval {
            reason: "shell.run requires approval by default".into(),
            hint: None,
            constrained_input: None,
        };
    }

    if matches!(input.risk_level, RiskLevel::Critical) {
        return Verdict::RequireApproval {
            reason: format!("tool {} classified critical", input.tool),
            hint: None,
            constrained_input: None,
        };
    }

    Verdict::Allow {
        reason: format!("risk={:?}", input.risk_level),
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

struct Constrained {
    args_json: String,
    dropped: String,
}

fn suggest_constrain(input: &GuardInput<'_>) -> Option<Constrained> {
    if input.tool != "shell.run" {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(input.arguments_json).ok()?;
    let cmd = v.get("command")?.as_str()?;
    let trimmed = cmd.trim();
    let rest = trimmed
        .strip_prefix("rm -rf ")
        .or_else(|| trimmed.strip_prefix("rm -fr "))?;
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }
    let dangerous = tokens.iter().find(|t| {
        let t = **t;
        t == "/" || t == "dist" || t.ends_with("/dist") || t.starts_with("~")
    })?;
    let dropped = dangerous.to_string();
    let remaining: Vec<&str> = tokens.iter().copied().filter(|t| *t != dropped).collect();
    if remaining.is_empty() {
        return None;
    }
    let new_cmd = format!("rm -rf {}", remaining.join(" "));
    let mut new_args = v.clone();
    if let Some(obj) = new_args.as_object_mut() {
        obj.insert("command".into(), serde_json::Value::String(new_cmd));
    }
    Some(Constrained {
        args_json: new_args.to_string(),
        dropped,
    })
}

/// The v0.1 alpha-demo policy seed.
pub fn alpha_demo_policy() -> PolicyDoc {
    PolicyDoc {
        name: "alpha-demo".into(),
        sensitivity_ceiling: Some(Sensitivity::High),
        tools: vec![
            ToolPolicy {
                tool: "shell.run".into(),
                risk_level: RiskLevel::Critical,
                require_approval: true,
            },
            ToolPolicy {
                tool: "file.write".into(),
                risk_level: RiskLevel::Medium,
                require_approval: false,
            },
            ToolPolicy {
                tool: "file.read".into(),
                risk_level: RiskLevel::Low,
                require_approval: false,
            },
        ],
        deny: vec![
            DenyRule {
                tool: "shell.run".into(),
                pattern: r"rm\s+-rf\s+.*\bdist\b".into(),
                reason: "rm -rf includes /dist — release artifacts".into(),
            },
            DenyRule {
                tool: "shell.run".into(),
                pattern: r"rm\s+-rf\s+/(?:\W|$)".into(),
                reason: "rm -rf on filesystem root".into(),
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constrain_hints_rm_dist() {
        let policy = alpha_demo_policy();
        let actor = ActorId::new();
        let input = GuardInput {
            actor_id: &actor,
            tool: "shell.run",
            arguments_json: r#"{"command":"rm -rf build dist"}"#,
            risk_level: RiskLevel::Critical,
            sensitivity: Sensitivity::Low,
        };
        let v = evaluate(&policy, &input);
        match v {
            Verdict::RequireApproval {
                hint,
                constrained_input,
                ..
            } => {
                assert!(hint.unwrap().contains("dist"));
                let s = constrained_input.unwrap();
                assert!(s.contains("rm -rf build"));
                assert!(!s.contains("dist"));
            }
            other => panic!("expected RequireApproval(constrain), got {other:?}"),
        }
    }

    #[test]
    fn shell_run_requires_approval_by_default() {
        let policy = alpha_demo_policy();
        let actor = ActorId::new();
        let input = GuardInput {
            actor_id: &actor,
            tool: "shell.run",
            arguments_json: r#"{"command":"ls"}"#,
            risk_level: RiskLevel::Low,
            sensitivity: Sensitivity::Low,
        };
        let v = evaluate(&policy, &input);
        assert!(matches!(v, Verdict::RequireApproval { .. }));
    }

    #[test]
    fn read_is_allowed() {
        let policy = alpha_demo_policy();
        let actor = ActorId::new();
        let input = GuardInput {
            actor_id: &actor,
            tool: "file.read",
            arguments_json: r#"{"path":"README.md"}"#,
            risk_level: RiskLevel::Low,
            sensitivity: Sensitivity::Public,
        };
        let v = evaluate(&policy, &input);
        assert!(matches!(v, Verdict::Allow { .. }));
    }
}
