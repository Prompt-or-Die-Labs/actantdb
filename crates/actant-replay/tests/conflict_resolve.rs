//! Tests for the per-projection conflict policy table (GAPS.md row #44).
//!
//! Covers:
//! - Higher HLC wins regardless of which side it sits on.
//! - HLC tie -> deterministic actor_id lexicographic tiebreak.
//! - Per-field policy maps name a different winner per field of the same
//!   row when two writers updated different fields concurrently.

use std::cmp::Ordering;

use actant_core::Hlc;
use actant_replay::{ConflictPolicy, HasHlc};

#[derive(Clone, Debug)]
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
fn higher_hlc_wins() {
    let p = ConflictPolicy::default_for_projections();
    let earlier = Row {
        hlc: Hlc::new(1_000, 5),
        actor: "act_alpha".into(),
    };
    let later = Row {
        hlc: Hlc::new(2_000, 0),
        actor: "act_zulu".into(),
    };

    assert_eq!(
        p.resolve("session", Some("title"), &later, &earlier),
        Ordering::Greater
    );
    assert_eq!(
        p.resolve("session", Some("title"), &earlier, &later),
        Ordering::Less
    );
}

#[test]
fn hlc_tie_breaks_on_actor_id_lex() {
    let p = ConflictPolicy::default_for_projections();
    let a = Row {
        hlc: Hlc::new(10, 3),
        actor: "act_alpha".into(),
    };
    let b = Row {
        hlc: Hlc::new(10, 3),
        actor: "act_zulu".into(),
    };
    // "act_alpha" < "act_zulu", so a < b => b wins.
    assert_eq!(
        p.resolve("memory", Some("approved_at"), &a, &b),
        Ordering::Less
    );
    assert_eq!(
        p.resolve("memory", Some("approved_at"), &b, &a),
        Ordering::Greater
    );
    // Self-compare is Equal.
    assert_eq!(
        p.resolve("memory", Some("approved_at"), &a, &a),
        Ordering::Equal
    );
}

#[test]
fn per_field_policy_picks_different_winners_per_field() {
    // Two writers concurrently edited the same `memory` row: writer X
    // bumped `approved_at` (high HLC), writer Y bumped `rejected_at`
    // (high HLC). Per-field LWW must let each field land on its own
    // winner — row-level LWW would lose one of them.
    let p = ConflictPolicy::default_for_projections();

    let approved_winner = Row {
        hlc: Hlc::new(5_000, 0),
        actor: "act_writer_x".into(),
    };
    let approved_loser = Row {
        hlc: Hlc::new(4_999, 0),
        actor: "act_writer_y".into(),
    };

    let rejected_winner = Row {
        hlc: Hlc::new(6_000, 0),
        actor: "act_writer_y".into(),
    };
    let rejected_loser = Row {
        hlc: Hlc::new(5_500, 0),
        actor: "act_writer_x".into(),
    };

    // approved_at: writer_x wins
    assert_eq!(
        p.resolve(
            "memory",
            Some("approved_at"),
            &approved_winner,
            &approved_loser
        ),
        Ordering::Greater
    );
    assert!(p.is_per_field("memory", "approved_at"));

    // rejected_at: writer_y wins
    assert_eq!(
        p.resolve(
            "memory",
            Some("rejected_at"),
            &rejected_winner,
            &rejected_loser
        ),
        Ordering::Greater
    );
    assert!(p.is_per_field("memory", "rejected_at"));

    // Sanity: per-field LWW lets the *same pair of writers* land on
    // different winners across fields when the caller hands `resolve`
    // the per-field HasHlc values. Writer X owns approved_at (higher HLC
    // when projected onto that field); writer Y owns rejected_at.
    let x_on_approved = Row {
        hlc: Hlc::new(7_000, 0),
        actor: "act_writer_x".into(),
    };
    let y_on_approved = Row {
        hlc: Hlc::new(6_000, 0),
        actor: "act_writer_y".into(),
    };
    let x_on_rejected = Row {
        hlc: Hlc::new(5_000, 0),
        actor: "act_writer_x".into(),
    };
    let y_on_rejected = Row {
        hlc: Hlc::new(8_000, 0),
        actor: "act_writer_y".into(),
    };
    // For approved_at: writer_x wins.
    assert_eq!(
        p.resolve(
            "memory",
            Some("approved_at"),
            &x_on_approved,
            &y_on_approved
        ),
        Ordering::Greater
    );
    // For rejected_at: writer_y wins (different winner, same pair of writers).
    assert_eq!(
        p.resolve(
            "memory",
            Some("rejected_at"),
            &x_on_rejected,
            &y_on_rejected
        ),
        Ordering::Less
    );
}

#[test]
fn row_level_fallback_for_unlisted_table() {
    let p = ConflictPolicy::default_for_projections();
    // `pubsub_message` isn't in the per-field map.
    assert!(!p.is_per_field("pubsub_message", "payload"));
    // Resolution still works -- it just degenerates to row-level LWW
    // (HLC + actor_id tiebreak).
    let a = Row {
        hlc: Hlc::new(100, 0),
        actor: "act_a".into(),
    };
    let b = Row {
        hlc: Hlc::new(101, 0),
        actor: "act_a".into(),
    };
    assert_eq!(p.resolve("pubsub_message", None, &b, &a), Ordering::Greater);
}
