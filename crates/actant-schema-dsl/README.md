# actant-schema-dsl

Parser and compiler for the `.actant` schema DSL.

The DSL lets a developer declare project schemas, commands, and workflows in one place:

```actant
table memory_candidate {
  id: ID primary
  text: String
  category: String
  confidence: Float
  sensitivity: Sensitivity
  status: ReviewStatus
  source_event_ids: [ID]
  created_at: Timestamp
}

command approve_memory {
  input {
    candidate_id: ID
    edited_text: String?
  }
  requires memory.approve
  emits memory_approved
}
```

Owns:

- Lexer + parser producing a typed AST.
- Validators: types resolve, foreign references resolve, no name collisions, sensitivity/visibility/risk values are members of the closed enums.
- Compilers:
  - `→ SQL`: produces `migrations/<NN>_<name>.sql` deltas against the workspace's prior schema.
  - `→ Rust types`: emits per-table row structs into `crates/<crate>/src/generated/`.
  - `→ Command stubs`: emits Python/TypeScript/Swift/Rust skeletons for project commands.
- Diff/migrate helpers used by `actant schema diff` and `actant schema migrate`.

Does **not** own: SQL execution (that's `actant-storage`'s migration runner).

See `agents/actant-schema-dsl.md` and `specs/adr/0009-schema-dsl.md`.
