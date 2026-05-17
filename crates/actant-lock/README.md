# actant-lock

Resource locks. Lease-based, expiry-bounded. Keys: `lock:file:<path>`, `lock:ticket:<id>`, `lock:memory:<id>`, `lock:workflow:<name>`, `lock:actor:<id>`. Prevents two agents from editing the same file, sending duplicate emails, or producing conflicting memory writes.

See `agents/actant-lock.md`.
