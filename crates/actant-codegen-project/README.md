# actant-codegen-project

Project-level code generators. Different from `actant-sdk-codegen` (which emits SDK types for *consumers* from server metadata) — this crate emits *project source* for developers building agents.

Generators:

- `actant generate command <name>` → command file + test
- `actant generate effect <name>`  → effect type + worker stub + test
- `actant generate worker <kind>`  → worker binary stub
- `actant generate agent <name>`   → agent skeleton with chosen capabilities
- `actant generate workflow <name>`→ workflow `.actant` file + invocation script

Owns:

- Language-aware templates for Python, TypeScript, Swift, Rust.
- Field-validation guardrails (refuses to overwrite without `--force`).
- Cross-file wiring (e.g. `generate worker` updates `actant.yaml`'s workers section).

Depends on `actant-schema-dsl` for type resolution; depends on `actant-templates` for the language-specific bodies.

See `agents/actant-codegen-project.md`.
