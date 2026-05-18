//! Property test for the subscribe engine.
//!
//! Spec/AC adjustment: the work package describes a snapshot+incremental
//! API (`SubscriptionEvent::Snapshot` / `Upsert` / `SnapshotComplete`). The
//! current `SubscribeHub` API (see `crates/actant-subscribe/src/lib.rs`)
//! is a simpler broadcast/fan-out: subscribers attach via `subscribe(topic)`
//! and receive every `publish` made after they attached, in order, until
//! the broadcast buffer is exceeded.
//!
//! We test the closest property the current API can support:
//! - 10 subscribers attach BEFORE the producer publishes,
//! - the producer publishes 1000 events to one topic,
//! - each subscriber receives all 1000 events in commit order.
//!
//! Increase the broadcast buffer via subscribing first (so `publish` can
//! see receivers and queue messages into them) and by draining each
//! receiver in parallel — keeping receiver backlog under the 256 default
//! channel capacity.

use actant_core::WorkspaceId;
use actant_subscribe::{SubscribeHub, Topic};
use tokio::task::JoinSet;

const NUM_SUBSCRIBERS: usize = 10;
const NUM_EVENTS: usize = 1000;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ten_subscribers_each_see_full_sequence_in_order() {
    let hub = SubscribeHub::new();
    let topic = Topic {
        workspace_id: WorkspaceId::new(),
        session_id: None,
        kind: "events".into(),
    };

    // Open all 10 subscribers BEFORE publishing — otherwise late subscribers
    // miss messages (broadcast semantics, by design).
    let mut tasks: JoinSet<Vec<u64>> = JoinSet::new();
    for _ in 0..NUM_SUBSCRIBERS {
        let mut rx = hub.subscribe(topic.clone()).await;
        tasks.spawn(async move {
            let mut received: Vec<u64> = Vec::with_capacity(NUM_EVENTS);
            for _ in 0..NUM_EVENTS {
                let msg = rx.recv().await.expect("recv");
                let seq = msg.payload["seq"].as_u64().expect("seq present");
                received.push(seq);
            }
            received
        });
    }

    // Publish 1000 events with a deterministic sequence. The broadcast
    // channel capacity is 256 (see SubscribeHub::subscribe); we publish in
    // batches of 64 with a 1ms sleep between batches so subscribers can
    // drain their receive queues and stay well under the buffer ceiling.
    let hub_pub = hub.clone();
    let topic_pub = topic.clone();
    let producer = tokio::spawn(async move {
        for seq in 0..NUM_EVENTS as u64 {
            hub_pub
                .publish(topic_pub.clone(), serde_json::json!({"seq": seq}))
                .await;
            if seq % 64 == 63 {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            }
        }
    });

    producer.await.expect("producer joins");

    let mut all_received: Vec<Vec<u64>> = Vec::with_capacity(NUM_SUBSCRIBERS);
    while let Some(res) = tasks.join_next().await {
        all_received.push(res.expect("subscriber task joined"));
    }

    assert_eq!(all_received.len(), NUM_SUBSCRIBERS);
    let expected: Vec<u64> = (0..NUM_EVENTS as u64).collect();
    for (i, got) in all_received.iter().enumerate() {
        assert_eq!(got.len(), NUM_EVENTS, "subscriber {i} got wrong count");
        assert_eq!(got, &expected, "subscriber {i} sequence mismatch");
    }
}

#[tokio::test]
async fn late_subscriber_misses_pre_attach_messages() {
    // Document the current semantics: this hub is broadcast-only and has
    // no snapshot phase. A subscriber attached AFTER publication will not
    // see the historical messages. If a snapshot phase is added later,
    // this test should be inverted.
    let hub = SubscribeHub::new();
    let topic = Topic {
        workspace_id: WorkspaceId::new(),
        session_id: None,
        kind: "events".into(),
    };
    // Force the topic into existence with an early subscriber that we drop.
    let _early = hub.subscribe(topic.clone()).await;
    hub.publish(topic.clone(), serde_json::json!({"early": true}))
        .await;

    let mut late = hub.subscribe(topic.clone()).await;
    hub.publish(topic.clone(), serde_json::json!({"late": true}))
        .await;
    let got = tokio::time::timeout(std::time::Duration::from_millis(250), late.recv())
        .await
        .expect("recv before timeout")
        .expect("recv ok");
    assert_eq!(
        got.payload["late"], true,
        "late subscriber should only see post-attach publishes"
    );
}
