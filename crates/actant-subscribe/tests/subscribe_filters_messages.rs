//! End-to-end: two subscribers on the same topic with different
//! predicates each see only the messages that match.
//!
//! Closes GAPS.md row #20.

use actant_core::WorkspaceId;
use actant_subscribe::{Predicate, SubscribeHub, Topic};
use serde_json::json;

#[tokio::test]
async fn two_predicates_partition_the_stream() {
    let hub = SubscribeHub::new();
    let topic = Topic {
        workspace_id: WorkspaceId::new(),
        session_id: None,
        kind: "events".into(),
    };

    // Subscriber A: only "shell" tool calls.
    let mut a = hub
        .subscribe_filtered(
            topic.clone(),
            Some(Predicate::Eq {
                field: "tool_name".into(),
                value: json!("shell"),
            }),
        )
        .await;
    // Subscriber B: only errors.
    let mut b = hub
        .subscribe_filtered(
            topic.clone(),
            Some(Predicate::Exists {
                field: "error".into(),
            }),
        )
        .await;
    // Subscriber C: no predicate — sees everything.
    let mut c = hub.subscribe_filtered(topic.clone(), None).await;

    hub.publish(topic.clone(), json!({"tool_name": "shell", "count": 1}))
        .await;
    hub.publish(topic.clone(), json!({"tool_name": "browser", "count": 2}))
        .await;
    hub.publish(
        topic.clone(),
        json!({"tool_name": "shell", "error": "boom"}),
    )
    .await;
    hub.publish(topic.clone(), json!({"unrelated": true})).await;

    // A: messages #0 (shell) and #2 (shell with error).
    let m1 = a.recv().await.unwrap();
    assert_eq!(m1.payload["count"], 1);
    let m2 = a.recv().await.unwrap();
    assert_eq!(m2.payload["error"], "boom");
    // No more matches.
    assert!(matches!(
        a.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));

    // B: only the error message.
    let m = b.recv().await.unwrap();
    assert_eq!(m.payload["error"], "boom");
    assert!(matches!(
        b.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));

    // C: all four messages, in order.
    for expected in [json!(1), json!(2), json!("boom"), json!(true)] {
        let m = c.recv().await.unwrap();
        // Inspect a field that's unique per message.
        let got = m
            .payload
            .get("count")
            .cloned()
            .or_else(|| m.payload.get("error").cloned())
            .or_else(|| m.payload.get("unrelated").cloned())
            .unwrap();
        assert_eq!(got, expected);
    }
}

#[tokio::test]
async fn complex_predicate_and_or_not_filters_correctly() {
    let hub = SubscribeHub::new();
    let topic = Topic {
        workspace_id: WorkspaceId::new(),
        session_id: None,
        kind: "events".into(),
    };

    // (tool == "shell" AND count > 5) OR NOT exists(suppressed)
    let pred = Predicate::Or(vec![
        Predicate::And(vec![
            Predicate::Eq {
                field: "tool".into(),
                value: json!("shell"),
            },
            Predicate::Gt {
                field: "count".into(),
                value: json!(5),
            },
        ]),
        Predicate::Not(Box::new(Predicate::Exists {
            field: "suppressed".into(),
        })),
    ]);
    let mut sub = hub.subscribe_filtered(topic.clone(), Some(pred)).await;

    // Sent in order; expected to be received in order, but only the
    // matching ones surface.
    let matching = [
        json!({"tool": "shell", "count": 10}), // matches via AND branch
        json!({"tool": "browser"}),            // matches via NOT-exists branch
    ];
    let dropped = [
        json!({"tool": "shell", "count": 1, "suppressed": true}), // AND fails, suppressed exists
        json!({"tool": "browser", "suppressed": true}),           // AND fails, suppressed exists
    ];

    // Interleave matching + dropped.
    hub.publish(topic.clone(), dropped[0].clone()).await;
    hub.publish(topic.clone(), matching[0].clone()).await;
    hub.publish(topic.clone(), dropped[1].clone()).await;
    hub.publish(topic.clone(), matching[1].clone()).await;

    let m1 = sub.recv().await.unwrap();
    assert_eq!(m1.payload, matching[0]);
    let m2 = sub.recv().await.unwrap();
    assert_eq!(m2.payload, matching[1]);
    assert!(matches!(
        sub.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}
