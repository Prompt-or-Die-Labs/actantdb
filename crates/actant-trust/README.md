# actant-trust

Behavioral trust profiles. Phase 3.

Owns:

- Signal extractors: tool success rate, policy violation rate, approval denial rate, memory correction rate, workflow completion rate, user feedback, eval scores, replay divergence.
- `compute_trust(actor, capability_area, signals) -> (score, confidence, sample_size)`.
- Periodic recalibration runner.
- Threshold-crossing detection → emits `trust_upgrade` / `trust_downgrade`.
- Admin `pin_trust` override path (audited).

Does **not** own: how trust affects Guard decisions. That coupling is in `actant-policy`, which reads trust as one of its inputs.

See `agents/actant-trust.md` and `specs/adr/0007-behavioral-trust.md`.
