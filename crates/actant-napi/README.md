# actant-napi

Node.js native addon. Compiled to a `.node` binary per platform and distributed inside `@actantdb/core` as optional `optionalDependencies` (Linux x64, Linux arm64, macOS x64, macOS arm64, Windows x64).

TypeScript developers do **not** install this crate. They install `@actantdb/core`. The native binaries are downloaded by `npm install` for their platform; the WASM fallback in `actant-wasm` handles anything not covered.

See [`/wedge/f2-f3-prevention.md`](../../wedge/f2-f3-prevention.md) §F2.
