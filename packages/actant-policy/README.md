# @actantdb/policy

Policy DSL + Guard verdict builders for `@actantdb/mastra` (and future wrappers).

Verdict shapes (defined once in [`crates/actant-contracts`](../../crates/actant-contracts), exported via [`@actantdb/types`](../actant-types)):

```ts
type PolicyVerdict =
  | { decision: "allow"; reason: string; policySnapshot: string }
  | { decision: "constrain"; reason: string; constrainedInput: unknown; policySnapshot: string }
  | { decision: "require_approval"; reason: string; risk: RiskLevel; policySnapshot: string }
  | { decision: "block"; reason: string; policySnapshot: string }
  | { decision: "halt"; reason: string; policySnapshot: string };
```

v0.1 policy capabilities:

- Per-tool risk class.
- Regex deny-list on arguments.
- Sensitivity ceiling per route.
- Built-in defaults for destructive shell, file write, email send.

See [`/wedge/killer-demo.md`](../../wedge/killer-demo.md) for the verdict path the demo exercises.
