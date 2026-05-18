//! Process-restart survival: a cron trigger that already fired must NOT
//! fire retroactively when the scheduler is reconstructed; the next fire
//! happens at the next scheduled time, and `last_fired_at` is respected.
//!
//! Covers AC: "Surviving a process restart: a paused-at-restart cron
//! trigger fires at its next scheduled time, not retroactively."
//!
//! AC adjustment: the current `Scheduler` keeps registrations in memory
//! (`Mutex<HashMap<id, Registration>>`); persisted-state recovery is the
//! host's job. To simulate "load from storage" we drop the first
//! scheduler and reconstruct a second one, re-registering the trigger
//! with the same `Registration` (carrying the `last_fired_at` from the
//! previous run). This is the closest available simulation of restart
//! recovery given the current API.

use actant_trigger::{FireAction, Registration, Scheduler, Trigger};
use time::OffsetDateTime;

#[tokio::test]
async fn restart_does_not_refire_already_fired_cron() {
    // Daily-at-00:00-UTC cron. Cold-start fires immediately because
    // `last_fired_at` is None and the schedule has crossed many times.
    let sched_a = Scheduler::new();
    sched_a
        .register(Registration {
            id: "t-restart".into(),
            name: "daily".into(),
            trigger: Trigger::Cron {
                expression: "0 0 0 * * * *".into(),
            },
            workflow_name: "daily-digest".into(),
            last_fired_at: None,
            enabled: true,
        })
        .await;

    let now = OffsetDateTime::now_utc().unix_timestamp();
    let first_fires = sched_a.tick(now).await;
    assert_eq!(first_fires.len(), 1, "expected one cron fire on cold start");
    assert!(
        matches!(&first_fires[0], FireAction::Fire { trigger_id, .. } if trigger_id == "t-restart")
    );

    // Snapshot the registration AFTER fire (carrying the new last_fired_at).
    let regs_after = sched_a.list().await;
    let reg_a = regs_after
        .into_iter()
        .find(|r| r.id == "t-restart")
        .expect("registration present");
    assert!(
        reg_a.last_fired_at.is_some(),
        "scheduler must record last_fired_at after firing"
    );

    // Simulate process restart: drop scheduler, build a fresh one, and
    // restore the registration as if loaded from storage.
    drop(sched_a);
    let sched_b = Scheduler::new();
    sched_b.register(reg_a.clone()).await;

    // A second tick immediately after restart at "now" must NOT refire —
    // the next scheduled time (tomorrow at 00:00 UTC) has not arrived.
    let restart_now = OffsetDateTime::now_utc().unix_timestamp();
    let fires_after_restart = sched_b.tick(restart_now).await;
    assert!(
        fires_after_restart.is_empty(),
        "restarting must NOT refire a trigger whose last_fired_at is after its previous scheduled time; got {fires_after_restart:?}"
    );

    // last_fired_at must be unchanged by the no-op tick.
    let regs_b = sched_b.list().await;
    let reg_b = regs_b
        .into_iter()
        .find(|r| r.id == "t-restart")
        .expect("registration present after restart");
    assert_eq!(
        reg_b.last_fired_at, reg_a.last_fired_at,
        "no-op tick must not mutate last_fired_at"
    );

    // Advancing the clock past the NEXT scheduled time should fire once.
    // For "0 0 0 * * * *" the next scheduled fire after `reg_a.last_fired_at`
    // is the following midnight — i.e. at most ~24h ahead. We push the
    // clock 25 hours forward and assert exactly one fire.
    let one_day_later = restart_now + 25 * 60 * 60;
    let fires_next_day = sched_b.tick(one_day_later).await;
    assert_eq!(
        fires_next_day.len(),
        1,
        "expected exactly one fire 25h after restart (the next scheduled time)"
    );
}
