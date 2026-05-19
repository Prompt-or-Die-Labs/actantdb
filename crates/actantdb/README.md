# actantdb

The all-in-one ActantDB crate. One dependency, every primitive.

## Quick start

```toml
[dependencies]
actantdb = "0.0"
```

```rust
use actantdb::storage::Storage;
use actantdb::policy::evaluate;
use actantdb::command::Engine;
```

## Just storage (feature-pruned)

```toml
[dependencies]
actantdb = { version = "0.0", default-features = false, features = ["storage"] }
```

## Mix and match

Add only the features you need:

```toml
actantdb = { version = "0.0", default-features = false, features = ["storage", "policy", "replay"] }
```

Every feature corresponds to one underlying `actant-*` crate; this crate
is only re-exports.

## Available features

storage, policy, command, replay, subscribe, auth, reliability, runtime,
objectstore, sync, workers, memory, effects, trigger, eval, embed,
capsule, trust, templates, audit-export, tenant, drift, compensation,
kernel, contracts.

See `Cargo.toml` for the full list.
