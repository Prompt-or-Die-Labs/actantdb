//! Policy types: rules, verdicts, approvals.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Risk;

/// A Guard verdict on a proposed tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PolicyVerdict {
    /// Allow the call as proposed.
    Allow {
        /// Human-readable reason for Studio display.
        reason: String,
        /// Snapshot hash of the policy that produced this verdict.
        policy_snapshot: String,
    },
    /// Allow a rewritten variant of the call.
    Constrain {
        /// Reason.
        reason: String,
        /// Snapshot hash.
        policy_snapshot: String,
        /// Rewritten arguments to use in place of the original.
        constrained_input: serde_json::Value,
        /// Short hint shown to the approver if approval is requested.
        hint: String,
    },
    /// Require explicit approval before the call executes.
    RequireApproval {
        /// Reason.
        reason: String,
        /// Snapshot hash.
        policy_snapshot: String,
        /// Optional constrain hint to offer the approver.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
        /// Optional constrained args the approver can choose to accept.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        constrained_input: Option<serde_json::Value>,
    },
    /// Block the call. The run continues but the tool call does not execute.
    Block {
        /// Reason.
        reason: String,
        /// Snapshot hash.
        policy_snapshot: String,
    },
    /// Halt the entire agent run.
    Halt {
        /// Reason.
        reason: String,
        /// Snapshot hash.
        policy_snapshot: String,
    },
}

impl PolicyVerdict {
    /// Returns a stable lowercase kind (for ledger payloads and matching).
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

/// A simple regex-based deny rule applied to tool arguments.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArgDenyRule {
    /// Tool name this rule applies to (e.g. "shell.run"). "*" for any tool.
    pub tool: String,
    /// Regular expression evaluated against the JSON-stringified args.
    pub pattern: String,
    /// Human reason; surfaced in the verdict.
    pub reason: String,
}

/// Per-tool risk classification entry.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolRiskEntry {
    /// Tool name.
    pub tool: String,
    /// Risk level assigned by the policy.
    pub risk: Risk,
    /// If true, every call requires approval regardless of args.
    #[serde(default)]
    pub require_approval: bool,
}

/// The v0.1 policy document. Small, opinionated, easy to read.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Policy {
    /// Per-tool risk class. Default risk if missing: `low`.
    #[serde(default)]
    pub tools: Vec<ToolRiskEntry>,
    /// Regex deny-list applied to args before risk evaluation.
    #[serde(default)]
    pub deny: Vec<ArgDenyRule>,
    /// Highest sensitivity allowed without approval.
    #[serde(default)]
    pub sensitivity_ceiling: Option<crate::Sensitivity>,
    /// Free-text label for Studio display.
    #[serde(default)]
    pub label: String,
}

/// A pending or recorded approval request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalRequest {
    /// Tool call id this approval is for.
    pub tool_call_id: String,
    /// Tool name.
    pub tool: String,
    /// Original arguments (pre-constrain).
    pub args: serde_json::Value,
    /// Constrain hint, if Guard suggested one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Constrained args the approver can accept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constrained_input: Option<serde_json::Value>,
    /// Reason from Guard.
    pub reason: String,
}

/// Approval outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Approved as-proposed.
    Approve {
        /// Approver identifier.
        approver: String,
        /// Approval scope (e.g. "once", "session").
        scope: String,
    },
    /// Approved the constrained variant.
    ApproveConstrained {
        /// Approver identifier.
        approver: String,
        /// Approval scope.
        scope: String,
        /// Final arguments accepted.
        accepted_input: serde_json::Value,
    },
    /// Denied.
    Deny {
        /// Approver identifier.
        approver: String,
        /// Free-text reason.
        reason: String,
    },
}
