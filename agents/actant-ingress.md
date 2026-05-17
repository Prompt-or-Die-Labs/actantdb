# Work package: `actant-ingress`

## Context

External event ingestion: webhooks (HMAC verified), email, calendar, filesystem watchers, MCP resources, A2A messages, manual CLI. Deduplication via `(source, dedupe_key)`. Triggers in `actant-trigger` consume these rows.

## Specs to read first

- `/specs/18-reliability-primitives.md` §7.
- `/specs/16-protocols.md` (MCP resources + A2A messages flow through here).

## Scope

```rust
pub struct IngressService { storage: Arc<actant_storage::Storage> }

pub enum Source { Webhook, Email, Calendar, Fs, Mcp, A2a, Manual }

impl IngressService {
    pub async fn ingest_webhook(&self, tx: &mut Transaction<'_>, headers: &HttpHeaders, body: &[u8], trigger_id: &TriggerId) -> Result<Option<IngressEventId>, IngressError>;
    pub async fn ingest_email(&self, tx: &mut Transaction<'_>, msg: EmailMessage) -> Result<Option<IngressEventId>, IngressError>;
    pub async fn ingest_fs(&self, tx: &mut Transaction<'_>, event: FsEvent) -> Result<Option<IngressEventId>, IngressError>;
    pub async fn ingest_manual(&self, tx: &mut Transaction<'_>, source: Source, event_type: &str, payload_ref: &str, dedupe_key: Option<&str>) -> Result<IngressEventId, IngressError>;
}
```

### Internal modules

```
crates/actant-ingress/src/
├── lib.rs
├── service.rs
├── hmac.rs                      (signature verification per trigger secret)
├── dedupe.rs                    (UNIQUE (source, dedupe_key) check)
├── adapters/                    (email, calendar, fs watch)
└── error.rs
```

### Tests

- HMAC verification: a webhook with bad signature records `signature_valid=0` and does NOT fire downstream triggers.
- Dedup: two webhook requests with identical `(source, dedupe_key)` produce one `ingress_event` row.
- Manual ingest skips signature verification but still dedups.

## Acceptance criteria

- [ ] Build/test/clippy green.
- [ ] Property: 100 duplicate webhook submissions produce exactly 1 ingress_event.

## Do NOT

- Do NOT fire workflow triggers on `signature_valid=0` events.
- Do NOT log raw payloads when `actant.sensitivity >= medium`; log hashes.

## Hand-off

`just ci`.
