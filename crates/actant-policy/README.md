# actant-policy

Guard — the policy / permissions / approvals engine.

Owns:

- `Guard::evaluate(actor, permission, resource, sensitivity, risk_level) -> Decision` where `Decision ∈ { Allow, AllowWithApproval, Deny }`.
- Loading and matching `authority_scope` rows for an actor.
- Resource-pattern matchers (file globs, host suffixes, browser domains, tool names).
- Sensitivity-ordering helpers (`public < low < medium < high < secret < regulated`).
- Visibility-set intersection helpers.
- Phase 1 scope: built-in default policy. Phase 2 adds custom policy bundles loaded from `policy.body_ref`.

Does **not** own: HTTP, command dispatch, side-effect execution. Just decisions.

See `agents/actant-policy.md` for the work package.
