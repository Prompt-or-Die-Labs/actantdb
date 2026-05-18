//! Success-criteria DSL for ActantDB evals.
//!
//! See `/specs/14-extended-primitives.md` §7 and `/agents/actant-eval.md` for the
//! versioned `SuccessCriteria` shape. Phase 4 v1 supports five operators:
//! `must_emit`, `must_not_emit`, `cost_le`, `latency_le_ms`, and `assert.jsonpath`.

use serde::{Deserialize, Serialize};

/// A single observed event in an eval window.
///
/// This is a minimal projection of `actant_storage::event_row` — just enough for
/// the criteria DSL to interpret. Larger systems can `From` into this shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event type, e.g. `tool_call_finished`.
    pub event_type: String,
    /// Optional cost charged to this event in dollars.
    #[serde(default)]
    pub cost: Option<f64>,
    /// Optional latency for this event in milliseconds.
    #[serde(default)]
    pub latency_ms: Option<u64>,
    /// Free-form payload for `assert.jsonpath` resolution.
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// Comparison operator for `Criterion::Assert`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AssertOp {
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// One criterion within an `all_of` list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Criterion {
    /// At least one event has `event_type == <name>`.
    MustEmit(String),
    /// No event has `event_type == <name>`.
    MustNotEmit(String),
    /// Sum of all `event.cost` is `<= <limit>`.
    CostLe(f64),
    /// Max of all `event.latency_ms` is `<= <limit>`.
    LatencyLeMs(u64),
    /// A jsonpath against `event.payload` (per matching event_type) compares to `value`.
    Assert {
        /// Limited path syntax: `$.foo.bar[0].baz`. Indexes use `[N]`.
        jsonpath: String,
        /// Comparison operator.
        op: AssertOp,
        /// Expected value (numbers and strings supported).
        value: serde_json::Value,
    },
}

/// All-of conjunction of criteria. (Phase 4 v1 has no `any_of`.)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SuccessCriteria {
    /// Every criterion must pass.
    #[serde(default)]
    pub all_of: Vec<Criterion>,
}

/// Result of evaluating `SuccessCriteria` against a list of events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalResult {
    /// True iff every criterion passed.
    pub passed: bool,
    /// One human-readable failure detail per failed criterion.
    pub failures: Vec<String>,
}

impl SuccessCriteria {
    /// Evaluate every criterion against `events`. Always returns; never panics on
    /// missing fields or bad jsonpath — those count as criterion failures.
    pub fn evaluate(&self, events: &[Event]) -> EvalResult {
        let mut failures = Vec::new();
        for c in &self.all_of {
            if let Err(detail) = evaluate_one(c, events) {
                failures.push(detail);
            }
        }
        EvalResult {
            passed: failures.is_empty(),
            failures,
        }
    }
}

fn evaluate_one(c: &Criterion, events: &[Event]) -> Result<(), String> {
    match c {
        Criterion::MustEmit(name) => {
            if events.iter().any(|e| e.event_type == *name) {
                Ok(())
            } else {
                Err(format!("must_emit: no event with type '{name}' observed"))
            }
        }
        Criterion::MustNotEmit(name) => {
            if events.iter().any(|e| e.event_type == *name) {
                Err(format!("must_not_emit: forbidden event '{name}' observed"))
            } else {
                Ok(())
            }
        }
        Criterion::CostLe(limit) => {
            let total: f64 = events.iter().filter_map(|e| e.cost).sum();
            if total <= *limit {
                Ok(())
            } else {
                Err(format!("cost_le: total cost {total} exceeds limit {limit}"))
            }
        }
        Criterion::LatencyLeMs(limit) => {
            let max = events.iter().filter_map(|e| e.latency_ms).max().unwrap_or(0);
            if max <= *limit {
                Ok(())
            } else {
                Err(format!(
                    "latency_le_ms: max latency {max}ms exceeds limit {limit}ms"
                ))
            }
        }
        Criterion::Assert { jsonpath, op, value } => evaluate_assert(jsonpath, *op, value, events),
    }
}

fn evaluate_assert(
    path: &str,
    op: AssertOp,
    expected: &serde_json::Value,
    events: &[Event],
) -> Result<(), String> {
    // Resolve the first event whose payload yields a value for `path`. If none, fail.
    let resolved = events
        .iter()
        .find_map(|e| resolve_path(&e.payload, path))
        .ok_or_else(|| format!("assert: jsonpath '{path}' did not resolve in any event"))?;

    if compare(&resolved, op, expected) {
        Ok(())
    } else {
        Err(format!(
            "assert: jsonpath '{path}' resolved to {resolved} but expected {op:?} {expected}"
        ))
    }
}

/// Tiny resolver for `$.foo.bar[0].baz` against a `serde_json::Value`.
///
/// Supported syntax: leading `$.`, dot-separated keys, `[N]` index suffix on any segment.
/// Unknown syntax or missing keys return `None`.
fn resolve_path(root: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let path = path.strip_prefix("$.").or_else(|| path.strip_prefix('$'))?;
    if path.is_empty() {
        return Some(root.clone());
    }
    let mut cur = root.clone();
    for raw_seg in path.split('.') {
        if raw_seg.is_empty() {
            return None;
        }
        // Split a segment like `events[0]` into key=`events`, indices=[0].
        let (key, rest) = match raw_seg.find('[') {
            Some(i) => raw_seg.split_at(i),
            None => (raw_seg, ""),
        };
        if !key.is_empty() {
            cur = cur.get(key).cloned()?;
        }
        let mut tail = rest;
        while !tail.is_empty() {
            tail = tail.strip_prefix('[')?;
            let end = tail.find(']')?;
            let idx: usize = tail[..end].parse().ok()?;
            cur = cur.get(idx).cloned()?;
            tail = &tail[end + 1..];
        }
    }
    Some(cur)
}

fn compare(left: &serde_json::Value, op: AssertOp, right: &serde_json::Value) -> bool {
    use serde_json::Value as V;
    // Numeric comparison when both sides are numbers.
    if let (Some(a), Some(b)) = (left.as_f64(), right.as_f64()) {
        return match op {
            AssertOp::Eq => a == b,
            AssertOp::Ne => a != b,
            AssertOp::Lt => a < b,
            AssertOp::Le => a <= b,
            AssertOp::Gt => a > b,
            AssertOp::Ge => a >= b,
        };
    }
    // String comparison when both sides are strings.
    if let (V::String(a), V::String(b)) = (left, right) {
        return match op {
            AssertOp::Eq => a == b,
            AssertOp::Ne => a != b,
            AssertOp::Lt => a < b,
            AssertOp::Le => a <= b,
            AssertOp::Gt => a > b,
            AssertOp::Ge => a >= b,
        };
    }
    // Fallback: structural equality only.
    match op {
        AssertOp::Eq => left == right,
        AssertOp::Ne => left != right,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_simple() {
        let v = json!({"a": {"b": 7}});
        assert_eq!(resolve_path(&v, "$.a.b"), Some(json!(7)));
    }

    #[test]
    fn resolve_index() {
        let v = json!({"xs": [{"k": "v"}, {"k": "w"}]});
        assert_eq!(resolve_path(&v, "$.xs[1].k"), Some(json!("w")));
    }

    #[test]
    fn resolve_missing() {
        let v = json!({"a": 1});
        assert!(resolve_path(&v, "$.b").is_none());
    }
}
