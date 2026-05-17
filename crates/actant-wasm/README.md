# actant-wasm

WASM build of the ActantDB kernel.

Distribution:
- Bundled inside `@actantdb/core` as the fallback when no `actant-napi` binary matches the host platform.
- Used by edge runtimes (Cloudflare Workers, Vercel Edge) and browser-side dev demos.

The WASM build supports a strict subset of the kernel surface: no native filesystem access, no native threads. Storage falls back to OPFS / IndexedDB in browser; in-memory for edge. Server mode is unavailable from the WASM surface — server runs only in the native paths.

See [`/wedge/f2-f3-prevention.md`](../../wedge/f2-f3-prevention.md) §F2.
