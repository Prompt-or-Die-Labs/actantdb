//! Exhaustive smoke test for the predicate evaluator.
//!
//! Covers every comparator, `Exists`, And/Or/Not, and the documented
//! missing-field semantics.

#![recursion_limit = "4096"]

use actant_subscribe::Predicate;
use serde_json::json;

fn root() -> serde_json::Value {
    json!({
        "tool_name": "shell",
        "count": 7,
        "tags": ["alpha", "beta"],
        "nested": { "ok": true, "score": 3.5 },
        "error": null,
    })
}

#[test]
fn eq_matches_exact_value() {
    let p = Predicate::Eq {
        field: "tool_name".into(),
        value: json!("shell"),
    };
    assert!(p.evaluate(&root()));

    let p = Predicate::Eq {
        field: "tool_name".into(),
        value: json!("browser"),
    };
    assert!(!p.evaluate(&root()));
}

#[test]
fn ne_is_inverse_of_eq_for_present_fields() {
    let p = Predicate::Ne {
        field: "tool_name".into(),
        value: json!("shell"),
    };
    assert!(!p.evaluate(&root()));

    let p = Predicate::Ne {
        field: "tool_name".into(),
        value: json!("browser"),
    };
    assert!(p.evaluate(&root()));
}

#[test]
fn ne_treats_missing_as_not_equal() {
    let p = Predicate::Ne {
        field: "missing".into(),
        value: json!("anything"),
    };
    assert!(p.evaluate(&root()));
}

#[test]
fn lt_le_gt_ge_on_numbers() {
    let v = root();
    assert!(Predicate::Lt {
        field: "count".into(),
        value: json!(8)
    }
    .evaluate(&v));
    assert!(!Predicate::Lt {
        field: "count".into(),
        value: json!(7)
    }
    .evaluate(&v));
    assert!(Predicate::Le {
        field: "count".into(),
        value: json!(7)
    }
    .evaluate(&v));
    assert!(Predicate::Gt {
        field: "count".into(),
        value: json!(6)
    }
    .evaluate(&v));
    assert!(!Predicate::Gt {
        field: "count".into(),
        value: json!(7)
    }
    .evaluate(&v));
    assert!(Predicate::Ge {
        field: "count".into(),
        value: json!(7)
    }
    .evaluate(&v));
}

#[test]
fn comparators_on_strings_use_lexicographic_order() {
    let v = root();
    assert!(Predicate::Lt {
        field: "tool_name".into(),
        value: json!("zzz")
    }
    .evaluate(&v));
    assert!(Predicate::Gt {
        field: "tool_name".into(),
        value: json!("aaa")
    }
    .evaluate(&v));
}

#[test]
fn type_mismatch_returns_false_no_coercion() {
    let v = root();
    let p = Predicate::Lt {
        field: "tool_name".into(),
        value: json!(5),
    };
    assert!(!p.evaluate(&v));
}

#[test]
fn missing_field_is_false_for_all_comparators_except_ne() {
    let v = root();
    for p in [
        Predicate::Eq {
            field: "nope".into(),
            value: json!("x"),
        },
        Predicate::Lt {
            field: "nope".into(),
            value: json!(0),
        },
        Predicate::Le {
            field: "nope".into(),
            value: json!(0),
        },
        Predicate::Gt {
            field: "nope".into(),
            value: json!(0),
        },
        Predicate::Ge {
            field: "nope".into(),
            value: json!(0),
        },
    ] {
        assert!(!p.evaluate(&v), "{:?}", p);
    }
}

#[test]
fn exists_distinguishes_missing_from_present_null() {
    let v = root();
    assert!(Predicate::Exists {
        field: "error".into()
    }
    .evaluate(&v));
    assert!(!Predicate::Exists {
        field: "nope".into()
    }
    .evaluate(&v));
    assert!(Predicate::Exists {
        field: "nested.ok".into()
    }
    .evaluate(&v));
}

#[test]
fn nested_field_paths_walk_objects_and_arrays() {
    let v = root();
    assert!(Predicate::Eq {
        field: "nested.ok".into(),
        value: json!(true)
    }
    .evaluate(&v));
    assert!(Predicate::Eq {
        field: "tags.0".into(),
        value: json!("alpha")
    }
    .evaluate(&v));
    assert!(Predicate::Eq {
        field: "tags.1".into(),
        value: json!("beta")
    }
    .evaluate(&v));
    assert!(!Predicate::Exists {
        field: "tags.7".into()
    }
    .evaluate(&v));
}

#[test]
fn and_short_circuits_and_empty_is_true() {
    let v = root();
    assert!(Predicate::And(vec![]).evaluate(&v));
    assert!(Predicate::And(vec![
        Predicate::Eq {
            field: "tool_name".into(),
            value: json!("shell")
        },
        Predicate::Gt {
            field: "count".into(),
            value: json!(0)
        },
    ])
    .evaluate(&v));
    assert!(!Predicate::And(vec![
        Predicate::Eq {
            field: "tool_name".into(),
            value: json!("shell")
        },
        Predicate::Eq {
            field: "tool_name".into(),
            value: json!("nope")
        },
    ])
    .evaluate(&v));
}

#[test]
fn or_short_circuits_and_empty_is_false() {
    let v = root();
    assert!(!Predicate::Or(vec![]).evaluate(&v));
    assert!(Predicate::Or(vec![
        Predicate::Eq {
            field: "tool_name".into(),
            value: json!("nope")
        },
        Predicate::Eq {
            field: "tool_name".into(),
            value: json!("shell")
        },
    ])
    .evaluate(&v));
    assert!(!Predicate::Or(vec![
        Predicate::Eq {
            field: "tool_name".into(),
            value: json!("nope")
        },
        Predicate::Eq {
            field: "tool_name".into(),
            value: json!("also-nope")
        },
    ])
    .evaluate(&v));
}

#[test]
fn not_inverts_inner() {
    let v = root();
    assert!(Predicate::Not(Box::new(Predicate::Eq {
        field: "tool_name".into(),
        value: json!("browser"),
    }))
    .evaluate(&v));
    assert!(!Predicate::Not(Box::new(Predicate::Eq {
        field: "tool_name".into(),
        value: json!("shell"),
    }))
    .evaluate(&v));
}

#[test]
fn true_and_false_constants() {
    let v = root();
    assert!(Predicate::True.evaluate(&v));
    assert!(!Predicate::False.evaluate(&v));
}

#[test]
fn serde_round_trip_preserves_ast() {
    let p = Predicate::And(vec![
        Predicate::Eq {
            field: "tool_name".into(),
            value: json!("shell"),
        },
        Predicate::Or(vec![
            Predicate::Gt {
                field: "count".into(),
                value: json!(0),
            },
            Predicate::Not(Box::new(Predicate::Exists {
                field: "missing".into(),
            })),
        ]),
    ]);
    let s = serde_json::to_string(&p).unwrap();
    let back: Predicate = serde_json::from_str(&s).unwrap();
    assert_eq!(p, back);
}
