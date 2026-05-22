# Work package: `actant-policy`

## Context

`actant-policy` is **Guard** — the authority, permissions, and approval-decision engine. It is the single point of truth for "can this actor do this?". Every command runs through it; every effect carries a `required_permission` that Guard verifies. Capsules and behavioral trust profiles live here because both are policy inputs, not standalone substrates.

## Specs to read first

- `/specs/05-security-model.md` — entire file, especially §2 (invariants), §3 (sensitivity), §4 (visibility), §5 (authority scopes), §6 (approval flow).
- `/specs/01-architecture.md` §"Guard".
- `/specs/02-data-model.sql` — `authority_scope`, `policy`, `approval_request` tables.
- `/specs/04-effect-protocol.md` §7 — effect types and their `required_permission` defaults.

## Scope

### Public API surface

```rust
pub struct Guard { storage: Arc<actant_storage::Storage> }

#[derive(Debug, Clone)]
pub enum Decision {
    Allow,
    AllowWithApproval { reason: String },
    Deny { reason: String },
}

pub struct Request<'a> {
    pub actor_id: &'a ActorId,
    pub workspace_id: &'a WorkspaceId,
    pub permission: &'a str,            // e.g. "shell.run", "file.read"
    pub resource: &'a str,              // e.g. "~/Projects/demo_repo/tests/test_math.py"
    pub sensitivity: Sensitivity,
    pub risk_level: RiskLevel,
}

impl Guard {
    pub fn new(storage: Arc<actant_storage::Storage>) -> Self;
    pub async fn evaluate(&self, req: Request<'_>) -> Result<Decision, GuardError>;

    // Match a resource against a stored resource_pattern.
    pub fn matches_pattern(pattern: &str, resource: &str, permission_kind: PatternKind) -> bool;
}

pub enum PatternKind { File, HttpHost, BrowserDomain, ToolName }
```

### Internal modules

```
crates/actant-policy/src/
├── lib.rs
├── error.rs
├── decision.rs            // Decision + Request types
├── evaluator.rs           // Guard::evaluate logic
├── patterns/
│   ├── mod.rs
│   ├── file_glob.rs       // shell-style globs with ~ expansion
│   ├── http_host.rs       // suffix match
│   ├── browser_domain.rs
│   └── tool_name.rs
└── builtin.rs             // built-in default policy (auto-approve low risk, etc.)
```

### Tests

- Sensitivity comparison ordering exhaustive.
- For each pattern kind, positive + negative cases.
- A scope with `expires_at` in the past does not match.
- A `revoked_at` scope does not match.
- For each `RiskLevel`, the right `Decision` shape with no explicit policy.
- For each effect type in `/specs/04-effect-protocol.md` §7, a default risk → decision mapping that matches the spec.
- Self-approval rejected for `medium`+.

## Acceptance criteria

- [ ] `cargo build -p actant-policy` zero warnings.
- [ ] `cargo test -p actant-policy` passes.
- [ ] `cargo clippy -p actant-policy -- -D warnings` passes.
- [ ] `evaluate` returns `Deny` if no matching scope; `Allow` if a matching scope exists and risk is `low`; `AllowWithApproval` if risk is `medium`+ without an explicit always-allow flag in policy.
- [ ] Every threat mitigation in `/specs/05-security-model.md` §7 that is named "Guard" has a test exercising the corresponding code path.

## Do NOT

- Do NOT write rows. Policy decisions are read-only; commands persist the decision via `audit_event`.
- Do NOT call out to a remote policy service. Phase 1 is built-in only.
- Do NOT add a generic `bool check(actor, str)` function. The typed `Request` exists so we cannot forget sensitivity or risk_level.
- Do NOT use `unsafe`.

## Hand-off

Run `just ci`.
