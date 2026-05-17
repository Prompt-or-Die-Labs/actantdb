# actant-sdk-codegen

Reads `/v1/metadata/commands` and `/v1/metadata/tables` from a reference server and emits typed clients for the supported SDK languages (`specs/09-sdk-design.md`).

Phase 1 emits: TypeScript types, Python pydantic models, Rust types. Phase 6 adds Swift `Codable` and the JVM family.

Binary: `actant-sdk-codegen` (`src/main.rs`). Usage:

```
actant-sdk-codegen --target ts    --out sdks/ts/src/generated/
actant-sdk-codegen --target py    --out sdks/python/actantdb/_generated.py
actant-sdk-codegen --target rust  --out sdks/rust/src/generated.rs
```

See `agents/actant-sdk-codegen.md` for the work package.
