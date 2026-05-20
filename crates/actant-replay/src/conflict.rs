//! Per-projection conflict resolution policy.
//!
//! Append-only event rows in `agent_event` are merge-free by definition.
//! Projections — "is this memory approved?", "what's the latest session
//! title?" — need a tie-breaking rule when two devices wrote concurrently
//! to the same row (or to the same field of the same row).
//!
//! Default policy (per `docs/IOS_EMBEDDING.md` §5):
//!
//! * `memory.{approved_at, rejected_at, last_verified_at}` — per-field LWW
//! * `session.{title, phase}` — per-field LWW
//! * `actor.display_name` — per-field LWW
//! * everything else — row-level LWW
//!
//! Per-field LWW lets two writers concurrently update *different* fields
//! of the same row without one stomping the other; row-level LWW is
//! simpler and is the right default for rows whose fields move together.
//!
//! Tiebreaker when HLCs compare equal: `actor_id` lexicographic order.
//! Documented in `docs/SYNC_DESIGN.md` §"Idempotency + conflict freedom".

use std::cmp::Ordering;
use std::collections::HashMap;

use actant_core::Hlc;

/// Anything carrying an HLC stamp and producing actor can be resolved by
/// the policy. The actor is used as the deterministic tiebreaker when
/// the HLCs compare equal.
pub trait HasHlc {
    /// HLC stamp of this row's most recent write.
    fn hlc(&self) -> Hlc;
    /// Producing actor id — used as a deterministic tiebreaker.
    fn actor_id(&self) -> &str;
}

/// Per-projection conflict policy.
///
/// `per_field_lww[table]` lists the columns of `table` that should be
/// merged at the field level. Any column not in the list (or any column
/// of a table not in the map) falls back to row-level LWW.
#[derive(Debug, Clone, Default)]
pub struct ConflictPolicy {
    /// Mapping from table name to the list of fields that get per-field LWW.
    pub per_field_lww: HashMap<&'static str, Vec<&'static str>>,
}

impl ConflictPolicy {
    /// Default substrate policy. See module docs.
    pub fn default_for_projections() -> Self {
        let mut per_field_lww: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
        per_field_lww.insert(
            "memory",
            vec!["approved_at", "rejected_at", "last_verified_at"],
        );
        per_field_lww.insert("session", vec!["title", "phase"]);
        per_field_lww.insert("actor", vec!["display_name"]);
        Self { per_field_lww }
    }

    /// Empty policy — every table falls back to row-level LWW.
    pub fn row_level_only() -> Self {
        Self {
            per_field_lww: HashMap::new(),
        }
    }

    /// Returns `true` if `field` of `table` is governed by per-field LWW.
    pub fn is_per_field(&self, table: &str, field: &str) -> bool {
        self.per_field_lww
            .get(table)
            .map(|fs| fs.contains(&field))
            .unwrap_or(false)
    }

    /// Resolve which of two `HasHlc` values wins.
    ///
    /// * `Ordering::Greater` — `a` wins (apply `a`'s value).
    /// * `Ordering::Less`    — `b` wins.
    /// * `Ordering::Equal`   — HLC + actor_id both tied; caller may treat as
    ///   "either is fine; pick one deterministically" (typically `a`).
    ///
    /// The `table` / `field` arguments are passed for symmetry with the
    /// caller's call site but the underlying comparison is just
    /// `(a.hlc(), a.actor_id())` vs `(b.hlc(), b.actor_id())`. Per-field
    /// LWW is realised by the caller constructing a `HasHlc` value whose
    /// `hlc()` reflects the most recent write *to that field*; the
    /// `per_field_lww` map (queried via [`Self::is_per_field`]) declares
    /// which `(table, field)` pairs are eligible for that treatment.
    /// Anything not in the map should be built as a row-level `HasHlc`
    /// (single HLC per row).
    pub fn resolve<T: HasHlc>(&self, _table: &str, _field: Option<&str>, a: &T, b: &T) -> Ordering {
        match a.hlc().cmp(&b.hlc()) {
            Ordering::Equal => a.actor_id().cmp(b.actor_id()),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Row {
        hlc: Hlc,
        actor: String,
    }

    impl HasHlc for Row {
        fn hlc(&self) -> Hlc {
            self.hlc
        }
        fn actor_id(&self) -> &str {
            &self.actor
        }
    }

    #[test]
    fn default_policy_lists_documented_fields() {
        let p = ConflictPolicy::default_for_projections();
        assert!(p.is_per_field("memory", "approved_at"));
        assert!(p.is_per_field("memory", "rejected_at"));
        assert!(p.is_per_field("memory", "last_verified_at"));
        assert!(p.is_per_field("session", "title"));
        assert!(p.is_per_field("session", "phase"));
        assert!(p.is_per_field("actor", "display_name"));
        assert!(!p.is_per_field("memory", "text")); // not in list -> row LWW
        assert!(!p.is_per_field("session", "status")); // not in list -> row LWW
    }

    #[test]
    fn higher_hlc_wins() {
        let p = ConflictPolicy::default_for_projections();
        let a = Row {
            hlc: Hlc::new(2_000, 0),
            actor: "act_alpha".into(),
        };
        let b = Row {
            hlc: Hlc::new(1_000, 99),
            actor: "act_zulu".into(),
        };
        assert_eq!(
            p.resolve("session", Some("title"), &a, &b),
            Ordering::Greater
        );
    }
}
