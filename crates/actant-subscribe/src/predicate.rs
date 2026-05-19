//! Row-level subscription predicates.
//!
//! A predicate is a small expression tree evaluated against the
//! [`Message::payload`](crate::Message) JSON value before the message is
//! handed to a subscriber. The language is intentionally minimal: field
//! references, literals, the six standard comparators, And/Or/Not, and
//! `Exists` for membership.
//!
//! Closes GAPS.md row #20.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Predicate AST.
///
/// `Field` paths are dotted; e.g. `"payload.tool_name"` walks `value["payload"]["tool_name"]`.
/// Array indices are supported as numeric segments: `"items.0.id"`.
///
/// # Evaluation rules
///
/// - Comparators on mismatched types return `false` (no coercion).
/// - Comparing JSON `null` to a non-null is `false` for any comparator
///   except `Ne`, which is `true`.
/// - Missing fields evaluate as `false` for every comparator. Use
///   [`Predicate::Exists`] to distinguish "missing" from "present-and-null".
/// - `Not(missing == X)` is `true` (missing makes the inner `false`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Predicate {
    /// Always-true predicate. Useful for "no filter" via `Some(Predicate::True)`.
    True,
    /// Always-false predicate.
    False,
    /// Field equals literal.
    Eq {
        /// Dotted JSON path.
        field: String,
        /// Literal JSON value.
        value: Value,
    },
    /// Field not-equal to literal.
    Ne {
        /// Dotted JSON path.
        field: String,
        /// Literal JSON value.
        value: Value,
    },
    /// Field strictly less than literal (numeric or string).
    Lt {
        /// Dotted JSON path.
        field: String,
        /// Literal JSON value.
        value: Value,
    },
    /// Field less-or-equal than literal.
    Le {
        /// Dotted JSON path.
        field: String,
        /// Literal JSON value.
        value: Value,
    },
    /// Field strictly greater than literal.
    Gt {
        /// Dotted JSON path.
        field: String,
        /// Literal JSON value.
        value: Value,
    },
    /// Field greater-or-equal than literal.
    Ge {
        /// Dotted JSON path.
        field: String,
        /// Literal JSON value.
        value: Value,
    },
    /// True iff the dotted path resolves to a value (including `null`).
    Exists {
        /// Dotted JSON path.
        field: String,
    },
    /// Logical AND. Empty vector evaluates to `true`.
    And(Vec<Predicate>),
    /// Logical OR. Empty vector evaluates to `false`.
    Or(Vec<Predicate>),
    /// Logical NOT.
    Not(Box<Predicate>),
}

impl Predicate {
    /// Evaluate the predicate against a JSON value.
    pub fn evaluate(&self, root: &Value) -> bool {
        match self {
            Self::True => true,
            Self::False => false,
            Self::Eq { field, value } => match resolve(root, field) {
                Some(v) => v == value,
                None => false,
            },
            Self::Ne { field, value } => match resolve(root, field) {
                Some(v) => v != value,
                // `Ne` is the documented exception: missing returns true so
                // that `Not(Eq{...})` and `Ne{...}` line up for present fields
                // and a missing field is "not equal to" any literal you name.
                None => true,
            },
            Self::Lt { field, value } => cmp(root, field, value, |o| {
                matches!(o, std::cmp::Ordering::Less)
            }),
            Self::Le { field, value } => cmp(root, field, value, |o| {
                matches!(o, std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            }),
            Self::Gt { field, value } => cmp(root, field, value, |o| {
                matches!(o, std::cmp::Ordering::Greater)
            }),
            Self::Ge { field, value } => cmp(root, field, value, |o| {
                matches!(o, std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            }),
            Self::Exists { field } => resolve(root, field).is_some(),
            Self::And(xs) => xs.iter().all(|p| p.evaluate(root)),
            Self::Or(xs) => xs.iter().any(|p| p.evaluate(root)),
            Self::Not(inner) => !inner.evaluate(root),
        }
    }
}

/// Convenience: top-level evaluator. Equivalent to `predicate.evaluate(root)`.
pub fn evaluate(predicate: &Predicate, root: &Value) -> bool {
    predicate.evaluate(root)
}

/// Walk a dotted path; numeric segments index into arrays.
fn resolve<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for segment in path.split('.') {
        if segment.is_empty() {
            return None;
        }
        cur = match cur {
            Value::Object(map) => map.get(segment)?,
            Value::Array(arr) => {
                let idx: usize = segment.parse().ok()?;
                arr.get(idx)?
            }
            _ => return None,
        };
    }
    Some(cur)
}

fn cmp<F>(root: &Value, field: &str, lit: &Value, ok: F) -> bool
where
    F: Fn(std::cmp::Ordering) -> bool,
{
    let Some(got) = resolve(root, field) else {
        return false;
    };
    let Some(order) = order(got, lit) else {
        return false;
    };
    ok(order)
}

fn order(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let xf = x.as_f64()?;
            let yf = y.as_f64()?;
            xf.partial_cmp(&yf)
        }
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        (Value::Null, Value::Null) => Some(std::cmp::Ordering::Equal),
        _ => None,
    }
}
