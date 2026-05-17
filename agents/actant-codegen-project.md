# Work package: `actant-codegen-project`

## Context

`actant generate ...` family. Produces project source — commands, effects, workers, agents, workflows — into a scaffolded ActantDB project. Different from `actant-sdk-codegen` (which produces SDK client types from server metadata).

## Specs to read first

- `/planning/cli-design.md` §"Generators".
- `/specs/03-command-spec.md` for the canonical command shape.
- `/specs/04-effect-protocol.md` for effect types.
- `/agents/actant-schema-dsl.md` for the parser of the project's `.actant` files.

## Scope

### Generators

```
actant generate command  <name>                          → commands/<name>.{py|ts|swift|rs} + tests/test_<name>.{ext}
actant generate effect   <type>                          → effects/<type>.actant + workers/<type>_worker.{ext} + tests
actant generate worker   <kind> [--sandbox <profile>]    → workers/<kind>_worker.{ext} + tests + actant.yaml update
actant generate agent    <name> [--with <feat,...>]      → agents/<name>.{ext}
actant generate workflow <name> [--template <kind>]      → workflows/<name>.actant + invocation script
```

### Public API

```rust
pub trait Generator {
    type Args: serde::de::DeserializeOwned;
    fn generate(args: Self::Args, project: &Project) -> Result<GeneratedFiles, CodegenError>;
}

pub struct Project { /* loaded from actant.yaml + schema/ */ pub language: Language, pub root: PathBuf, pub schema: actant_schema_dsl::Schema }

pub struct GeneratedFiles { pub files: Vec<(PathBuf, String)>, pub touched: Vec<(PathBuf, String)>, pub post_hooks: Vec<PostHook> }

pub struct CommandGen;
pub struct EffectGen;
pub struct WorkerGen;
pub struct AgentGen;
pub struct WorkflowGen;
```

### Language-aware bodies

Per generator + per language, a template string. Stored as `include_str!` of files under `crates/actant-codegen-project/templates/`. Examples:

- `commands/python/default.py`
- `commands/typescript/default.ts`
- `commands/swift/default.swift`
- `commands/rust/default.rs`

### Guard rails

- Refuse to overwrite without `--force`.
- Detect schema collisions: a generated command that emits an event not declared anywhere fails validation.
- Update `actant.yaml` only via a structured editor (preserves comments and order).

### Internal modules

```
crates/actant-codegen-project/src/
├── lib.rs
├── project.rs
├── command_gen.rs
├── effect_gen.rs
├── worker_gen.rs
├── agent_gen.rs
├── workflow_gen.rs
├── language.rs
└── error.rs
crates/actant-codegen-project/templates/    (embedded language bodies)
```

### Tests

- Each generator produces files; the rendered project still parses (`actant schema validate`).
- `--force` overwrites; default mode refuses.
- Adding a command and re-running `actant schema apply` produces a clean migration that compiles.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] Every generator produces at least one working test case in each of Python and TypeScript.
- [ ] Swift + Rust language paths exist with at least one fixture each (Phase 1 minimum; coverage expands in Phase 2+).

## Do NOT

- Do NOT silently overwrite developer code.
- Do NOT couple to the SDK code generator. They share no code paths.
- Do NOT emit code that imports unstable internal APIs.

## Hand-off

`just ci`. Then in a scaffolded project: `actant generate command demo_cmd && actant schema apply` and verify the generated test passes.
