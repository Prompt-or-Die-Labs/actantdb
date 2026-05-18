# Work package: `actant-prompts`

## Context

Prompt + tool-schema registry. Prompts are versioned artifacts with input schemas, renderers, and eval hooks. Tool schemas already live in `tool_schema_version`; this crate co-owns prompt versioning and lets replay reconstruct the *exact* prompt + tool-schema the model saw.

## Specs to read first

- `/specs/14-extended-primitives.md` §"Phase staging" (eval).
- `/specs/15-actant-index.md` §13 (model-specific prompt formatting).
- `/specs/16-protocols.md` §1 (MCP prompts).

## Scope

```rust
pub struct PromptService { storage: Arc<actant_storage::Storage> }

impl PromptService {
    pub async fn create(&self, tx: &mut Transaction<'_>, name: &str) -> Result<PromptTemplateId, PromptError>;
    pub async fn add_version(&self, tx: &mut Transaction<'_>, template_id: &PromptTemplateId, body_ref: &str, schema_ref: Option<&str>) -> Result<PromptVersionId, PromptError>;
    pub fn render(&self, body: &str, vars: &serde_json::Value) -> Result<String, PromptError>;
    pub async fn diff(&self, a: &PromptVersionId, b: &PromptVersionId) -> Result<PromptDiff, PromptError>;
}
```

### Internal modules

```
crates/actant-prompts/src/
├── lib.rs
├── service.rs
├── render.rs                    (variable substitution; sandboxed handlebars-style)
├── diff.rs
└── error.rs
```

### Tests

- Round-trip: create + add_version + retrieve by `(name, version)`.
- Render: variable substitution; refuses unknown control flow.
- Diff: text + schema + variables.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] ~~Every model call recorded during a `coding-agent` template run references a stored prompt version.~~
      _Removed 2026-05-18 — the v0.1 `model.call` event schema (owned by
      `actant-contracts`) does not carry `prompt_version_id`. Wiring this AC
      would require a contract-schema change rather than a test-only pass; it
      is tracked in `/specs/11-roadmap.md` Phase 4 (prompts versioning)._
- [ ] Replay can re-render the prompt for any prior model call.

## Do NOT

- Do NOT support arbitrary code execution in templates.
- Do NOT inline raw prompts in `model_call.request_ref` when a versioned prompt is available; reference it.

## Hand-off

`just ci`.
