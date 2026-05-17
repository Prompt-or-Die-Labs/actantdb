# Work package: `actant-schema-dsl`

## Context

The `.actant` schema DSL is the per-project schema language. It compiles to (1) SQL migrations the workspace's SQLite/Postgres needs, (2) Rust/Python/TypeScript/Swift types in the developer's project, and (3) command stubs.

This is **per-project** schema (the user's tables and commands), not the **ActantDB core** schema (which is in `/migrations/0001_initial.sql` and `/migrations/0002_extended_primitives.sql`). The core schema is fixed; this DSL lets a developer add domain-specific projection rows alongside it.

## Specs to read first

- `/specs/adr/0009-schema-dsl.md` — the design decision.
- `/specs/02-data-model.sql` — the type vocabulary (Sensitivity, Visibility, etc.) the DSL must respect.
- `/specs/03-command-spec.md` — every `command` block in the DSL produces a project command that follows the same lifecycle.

## Scope

### DSL grammar (v1)

```
file        := decl*
decl        := table | command | workflow | enum
table       := "table" Ident "{" field+ "}"
field       := Ident ":" type modifier*
type        := Ident ( "[" "]" )? ( "?" )?
modifier    := "primary" | "default" expr
command     := "command" Ident "{" "input" "{" field+ "}" "requires" Ident "emits" Ident "}"
workflow    := "workflow" Ident "{" node+ edge+ "}"      # subset; see /agents/phase-4-extensions.md
enum        := "enum" Ident "{" Ident ( "," Ident )* "}"
```

### Public API

```rust
pub struct Schema { pub tables: Vec<TableDecl>, pub commands: Vec<CommandDecl>, pub workflows: Vec<WorkflowDecl>, pub enums: Vec<EnumDecl> }

pub struct Parser;
impl Parser {
    pub fn parse(src: &str) -> Result<Schema, ParseError>;
    pub fn parse_dir(path: &Path) -> Result<Schema, ParseError>;
}

pub struct Validator;
impl Validator {
    pub fn validate(s: &Schema) -> Result<(), Vec<ValidationError>>;
}

pub struct Compiler;
impl Compiler {
    pub fn to_sql(prior: Option<&Schema>, current: &Schema) -> Result<String, CompileError>;
    pub fn to_rust(s: &Schema) -> Result<String, CompileError>;
    pub fn to_python(s: &Schema) -> Result<String, CompileError>;
    pub fn to_typescript(s: &Schema) -> Result<String, CompileError>;
    pub fn to_swift(s: &Schema) -> Result<String, CompileError>;
}
```

### Type vocabulary

Built-ins:

```
ID         (TEXT)
String     (TEXT)
Int        (INTEGER)
Float      (REAL)
Bool       (INTEGER 0/1)
Timestamp  (TEXT RFC3339)
JSON       (TEXT JSON)
[T]        (array, stored as JSON array of T)
T?         (nullable)
```

Closed-set built-ins (must match `actant-core` enums):

```
Sensitivity   = public | low | medium | high | secret | regulated
Visibility    = local_model_allowed | cloud_model_allowed | human_only | never_model | never_sync
RiskLevel     = low | medium | high | critical
ReviewStatus  = proposed | pending_review | approved | rejected | edited
```

User-defined `enum` declarations are allowed but cannot shadow the built-ins.

### Validation rules

- Every table has exactly one `primary` field.
- Foreign refs (named with `Ident.Ident`) resolve.
- No table name collides with the core schema's tables (the validator carries the core's table list).
- Closed-set enums (Sensitivity, etc.) cannot be re-declared.
- Every `command` `requires` permission string follows `<verb>.<resource>` or `<verb>.<resource>:<scope>`.
- Every `command` `emits` event is referenced in core EventType OR is a project-defined event in the same file.

### Diff semantics

`Compiler::to_sql(prior, current)`:

- Adds new tables.
- Adds new columns with `DEFAULT` (mandatory for non-null).
- Renames are explicit via `@renamed_from "old"` annotation; the compiler refuses ambiguous renames.
- Drops are explicit via `@retired_in_version N`.

### Internal modules

```
crates/actant-schema-dsl/src/
├── lib.rs
├── ast.rs                       (Schema + nodes)
├── parser.rs                    (LALRPOP-free hand-rolled or pest-based; pick at impl time)
├── validator.rs
├── compile/
│   ├── mod.rs
│   ├── sql.rs
│   ├── rust.rs
│   ├── python.rs
│   ├── typescript.rs
│   └── swift.rs
├── diff.rs
└── error.rs
```

### Tests

- Parser round-trip on every example in `/templates/coding-agent/schema/`.
- Validator rejects duplicate primary keys, unresolved refs, shadowed built-ins.
- SQL diff: adding a column produces `ALTER TABLE ADD COLUMN ... DEFAULT ...`.
- Rust output compiles when included in a downstream crate.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] Parses every `.actant` file in the bundled `coding-agent` template.
- [ ] Generated SQL passes `actant schema validate` against a real workspace.
- [ ] Generated Rust types pass `cargo check` in a downstream crate.
- [ ] Generated TS types pass `tsc --strict --noEmit`.

## Do NOT

- Do NOT extend the core type vocabulary in v1. Keep the built-ins fixed; ADR-required.
- Do NOT introduce inheritance or modules in v1. Flat decls only.
- Do NOT emit migrations that drop columns without an explicit `@retired_in_version`.

## Hand-off

`just ci`. Then run `actant schema validate` + `actant schema apply` against a freshly scaffolded `coding-agent` project.
