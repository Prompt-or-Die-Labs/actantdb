//! Snapshot-purge orphans dependent eval cases — placeholder gate for the
//! "snapshot-purge orphans dependent eval cases with `eval_case_orphaned`
//! event" half of the audit row on `agents/phase-5-extensions.md`.
//!
//! Per `/specs/14-extended-primitives.md` §13 the substrate is supposed
//! to emit an `eval_case_orphaned` event (and flip `eval_case.enabled` to
//! 0) when an eval's checkpoint is purged. Today the spec is the only
//! place that name appears: no code path emits the event, no column
//! tracks the flag, and `actant-eval` has no checkpoint-anchored state.
//!
//! The work-package instruction was: "if the orphan event/flag is
//! missing entirely, write the test with `#[ignore]` and note in
//! `agents/phase-5-extensions.md`." We do that here. The test body below
//! is the shape the test SHOULD take once the orphan signal lands; it is
//! intentionally not wired to any code so that the day someone implements
//! the orphan path they can delete the `#[ignore]` line and immediately
//! see whether it passes.

#[ignore = "orphan signal not yet emitted; see agents/phase-5-extensions.md"]
#[tokio::test]
async fn snapshot_purge_orphans_dependent_eval_cases() {
    // INTENDED SHAPE (pseudo):
    //   1. Create a session with one model_call event.
    //   2. checkpoint(eid) -> cp_id.
    //   3. Create an eval_case anchored to cp_id.
    //   4. DELETE FROM replay_checkpoint WHERE id = cp_id (snapshot purge).
    //   5. Assert one of:
    //        - SELECT enabled FROM eval_case WHERE id = ... returns 0, OR
    //        - SELECT 1 FROM agent_event WHERE event_type =
    //              'eval_case_orphaned' AND payload_inline LIKE %cp_id%
    //          returns at least one row.
    //
    // Neither column nor event exists today. When the orphan path is
    // implemented, remove the `#[ignore]` attribute above.
    panic!("orphan signal not yet emitted; remove #[ignore] when it is");
}
