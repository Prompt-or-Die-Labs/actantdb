# UI + Auth Design — Linking-Code Flow

Status: design, not implementation. Land behind a feature flag and gate behind
a `tests/spec_NN_verification.rs` regression test.

## 1. Audit of current state

**Studio (Node, `@actantdb/studio`).** `src/server.ts` runs `node:http`,
**always binds `127.0.0.1`**, serves `/`, `/studio.css`, `/studio.js`, plus
`/api/info|events|approvals|approvals/decide|replay`. `src/cli.ts cmdStudio`
opens the ledger and prints `Studio listening on http://127.0.0.1:4555` — no
browser open, no auth, no session, no first-run flow. `ui/index.html` renders
Runs / Timeline / Detail + Replay modal; no login screen, no identity.

**Rust server (`actantdb-server`, `crates/actant-server`).** `bin/server.rs`
parses `--bind` (default `127.0.0.1:4555`), `--db`, `--tls-*`. It does **not**
call `AppState::with_auth(...)`. `bootstrap()` seeds `ws_default` +
`act_system` when the DB is empty. `enforce_auth` (lib.rs:496) already exists
and enforces `claims.iss == workspace_id` when `auth_secret` is `Some`, but
since the binary never sets it, **auth is effectively off in production
today.** The Rust server does not serve the Studio UI — two unrelated surfaces.

**actant-auth (`crates/actant-auth`).** `Claims { sub, iss, roles[], iat, exp }`,
HS256 only (`sign` / `verify` / `principal_from_claims`). `oidc.rs` has RS256
+ JWKS for Phase 6.5; not wired. No session table, no password hashing, no
first-run detection, no linking codes.

## 2. Goals and non-goals

**Goals.** (1) Loopback bind: open browser, see Studio, no password. OS user
is the trust boundary. (2) Non-loopback bind: on first start print a one-time
**linking code**; first browser to paste it claims ownership and sets a
password. (3) After link, the Rust server serves the Studio UI directly
(single origin) and every gated endpoint is protected by a session cookie
that resolves to the same `Principal` `enforce_auth` already produces.
(4) The linking code rotates on every restart **until** ownership is claimed;
recovery is a separate `actantdb-server reset-password` subcommand. (5)
Existing HS256 Bearer auth for service accounts keeps working — sessions and
bearer tokens are two independent credentials on the same chokepoint.

**Non-goals.** OIDC for humans in v1 (`oidc.rs` stays for service tokens /
Phase 6.5 SSO). RBAC beyond `owner` (invite = fresh linking code).
Email/SMTP recovery (recovery is OS-local).

## 3. Trust model

| Bind | Mode | Auth |
|------|------|------|
| `127.0.0.1` or `::1`, any port | `local` | Skip session+bearer enforcement for **same-origin browser requests** (cookie omitted). Trust the OS user. |
| anything else (`0.0.0.0`, public IP, behind reverse proxy) | `remote` | Linking-code on first start; session cookie or bearer JWT thereafter. |

Detection: parse `--bind`, `IpAddr::is_loopback()` is the gate. `--force-auth`
/ `ACTANTDB_FORCE_AUTH=1` flips loopback into `remote` for reverse-proxy
users. Reverse-proxy footgun: when bind is loopback but `X-Forwarded-For`
arrives without `ACTANTDB_TRUST_PROXY=1`, return `403 reverse_proxy_detected,
use --force-auth`. Logged at boot.

## 4. Storage

Add one migration: `migrations/0004_auth.sql` (and parallel
`migrations/pg/0004_auth.sql`).

```sql
-- One row per workspace. NULL password_hash means "claimed but no password
-- set yet" (the link succeeded but the user hasn't picked one); the row is
-- only created the moment a linking code is redeemed.
CREATE TABLE workspace_owner (
    workspace_id    TEXT PRIMARY KEY REFERENCES workspace(id),
    actor_id        TEXT NOT NULL REFERENCES actor(id),
    email           TEXT,             -- optional, set with password
    password_hash   TEXT,             -- argon2id PHC string
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- One row per outstanding linking code. Single-use, short TTL. Code is
-- stored hashed (sha256, not Argon2 — speed matters, but the code is
-- ~80 bits of entropy, so a plain hash is fine).
CREATE TABLE link_code (
    code_hash       TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    expires_at      TEXT NOT NULL,     -- ISO-8601
    consumed_at     TEXT,
    consumed_by_ip  TEXT,
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_link_code_expires ON link_code(expires_at);

-- One row per active browser session.
CREATE TABLE session_token (
    token_hash      TEXT PRIMARY KEY,  -- sha256 of opaque token
    workspace_id    TEXT NOT NULL REFERENCES workspace(id),
    actor_id        TEXT NOT NULL REFERENCES actor(id),
    csrf_token      TEXT NOT NULL,     -- random, returned in body, sent back in X-CSRF
    issued_at       TEXT NOT NULL,
    expires_at      TEXT NOT NULL,
    last_seen_at    TEXT NOT NULL,
    revoked_at      TEXT,
    user_agent      TEXT,
    ip              TEXT
);
CREATE INDEX idx_session_workspace ON session_token(workspace_id);
```

The schema additions live in `actant-contracts` first (new types
`WorkspaceOwner`, `LinkCode`, `SessionToken`), then regenerate TS per the
binding rules.

Code format: `xxxx-xxxx-xxxx` (12 chars, base32 alphabet `ABCDEFGHJKMNPQRSTUVWXYZ23456789` — Crockford-style, no 0/O/1/I/L). 12 base32 chars
= 60 bits of entropy. Stored as `sha256(code_lowercased_no_dashes)`.

## 5. Linking-code flow

### 5.1 CLI UX

`actantdb-server` startup decides between three printouts.

**Local mode** (loopback, no `--force-auth`, no existing owner):

```
actantdb listening on http://127.0.0.1:4555
Open Studio: http://127.0.0.1:4555/studio
(local mode: no password required)
```

If `stdout` is a TTY and `ACTANTDB_NO_BROWSER` is unset, also spawn the
system browser at `/studio` (Node uses `open` package; Rust uses the `open`
crate, gated behind a feature so it's optional).

**Remote mode, unclaimed** (non-loopback bind, no row in `workspace_owner`):

```
actantdb listening on http://0.0.0.0:4555
A one-time linking code is required to claim this workspace:

    Code:    K7XQ-9MNP-2HTR
    Expires: 2026-05-18T17:02:00Z (15 minutes)

Open: http://<your-host>:4555/link
Or:   http://<your-host>:4555/link/K7XQ-9MNP-2HTR

(this code rotates on each restart until ownership is claimed)
```

The link code is generated **at startup** and held in memory + the
`link_code` table with 15-minute TTL. On restart-without-owner, the previous
unconsumed code is invalidated (cleared from the table) and a new one
generated. **Never logged** beyond the boot banner; not in `tracing::info!`
output.

**Remote mode, claimed**: regular startup banner only. No code printed.
Users hit `/login`.

### 5.2 Browser flow

1. User navigates to `http://host:4555/link` (or
   `http://host:4555/link/K7XQ-9MNP-2HTR`).
2. UI: large input field for the code (auto-filled from path if present),
   plus an explanation paragraph.
3. POST `/v1/auth/link` `{ "code": "K7XQ-9MNP-2HTR" }`. Server:
   - Normalizes (strip dashes, lowercase), hashes, looks up `link_code`.
   - 404 if no row; 410 if `expires_at` past; 409 if `consumed_at` set.
   - Atomically: insert into `workspace_owner` (no password yet), promote a
     new `actor` row with kind `Human` and a generated display name, mark
     `link_code.consumed_at`, issue a session token + CSRF token, set the
     session cookie.
4. Server responds `{ status: "linked", needs_password: true,
   csrf: "...", workspace_id, actor_id }`.
5. UI redirects to `/setup-password`. POST `/v1/auth/password`
   `{ "password": "..." }` writes `argon2id` hash via the `argon2` crate
   (params: m=64 MiB, t=3, p=4 — calibrate against constants in `argon2`
   defaults; never roll our own).
6. UI redirects to `/studio`.

Subsequent visits: `/login` shows email-or-blank + password. POST
`/v1/auth/login` verifies argon2, mints a new session token, sets the
cookie, returns CSRF + redirect target.

### 5.3 Session cookie

- Name: `actantdb_session`.
- `HttpOnly; SameSite=Lax; Path=/;` plus `Secure` if and only if the request
  arrived over HTTPS (detected via `Forwarded`/`X-Forwarded-Proto` only when
  `ACTANTDB_TRUST_PROXY=1`; otherwise the tls cert presence is the signal).
- Value: 256-bit random base64url. Server stores `sha256(value)`.
- TTL: 30 days, sliding (refresh `last_seen_at`; rotate token after 7 days
  via `Set-Cookie` on a successful request).
- CSRF: every `POST/PUT/DELETE` requires `X-CSRF-Token` header matching
  `session_token.csrf_token`. `GET` exempt. Bearer-JWT requests exempt
  (no cookie, no CSRF surface).

## 6. Replay / code-stuffing / brute-force defenses

- **Linking code**: 60 bits of entropy; 15-minute TTL; one redemption.
  Add **per-IP token bucket** on `/v1/auth/link`: 10 requests / 60 s,
  burst 3. Reuse `actant-throttle::Bucket` (already in `AppState`). On
  exhaust, return 429 and **do not** echo back whether the code was valid.
- **Login**: per-`(workspace_id, ip)` bucket: 10 / 60 s. After 5 consecutive
  failures, server pads the response with `tokio::time::sleep` to a uniform
  500ms.
- **Password**: argon2id with library defaults. PHC string stored as-is so
  parameters travel with the hash.
- **Cookie replay**: session token is opaque; lookup is by hash so a
  leaked DB row doesn't expose the cookie. Cookie rotation every 7 days
  bounds the replay window.
- **CSRF**: see above. Cookie-bearing requests must carry the CSRF header;
  pure JSON-API requests use `Authorization: Bearer ...` and have no
  cookie.
- **Setup race**: linking and password-set happen in a single transaction
  per step; `INSERT INTO workspace_owner` uses `INSERT OR IGNORE` and
  returns 409 if a second linker beats the first to commit. Only one owner
  can be claimed per workspace per linking code; subsequent owners require
  the existing owner to issue an invite (`POST /v1/auth/invite` generates a
  fresh linking code with TTL).
- **TLS off + public bind**: refuse to start, exit 1 with
  `error: refusing to bind non-loopback without --tls-cert/--tls-key (set
  ACTANTDB_INSECURE=1 to override)`. This is the single biggest footgun
  we can prevent statically.

## 7. Specific file changes

### 7.1 Rust — `crates/actant-contracts`

- Add types `WorkspaceOwner`, `LinkCode`, `SessionToken`, plus request/response
  shapes `LinkRequest`, `LinkResponse`, `LoginRequest`, `LoginResponse`,
  `PasswordSetRequest`. Run `cargo run -p actant-contracts --bin actant-contracts -- check-compat`
  and `codegen-ts`; commit Rust + regenerated TS in the same PR.

### 7.2 Rust — `crates/actant-auth/src/lib.rs`

Add to the public surface:

- `fn hash_password(plaintext: &str) -> Result<String, ActantError>` — argon2id.
- `fn verify_password(plaintext: &str, phc: &str) -> Result<bool, ActantError>`.
- `fn mint_session_token() -> (String /*opaque*/, String /*sha256*/, String /*csrf*/)`.
- `fn mint_link_code() -> (String /*display*/, String /*sha256*/)`.
- New module `session.rs`: `pub struct SessionStore { storage: Storage }` with
  `issue`, `lookup_by_token_hash`, `rotate`, `revoke`, `gc_expired`.
- New module `link.rs`: `pub struct LinkStore` with `issue`, `redeem`,
  `gc_expired`, `clear_unconsumed`.

### 7.3 Rust — `crates/actant-server/src/lib.rs`

- New routes (registered in `router`):
  - `POST /v1/auth/link` — redeem a linking code, issue session cookie.
  - `POST /v1/auth/password` — set password (requires session, requires
    `workspace_owner.password_hash IS NULL` OR a current-password proof).
  - `POST /v1/auth/login` — username/password → session.
  - `POST /v1/auth/logout` — revoke current session.
  - `POST /v1/auth/invite` — owner-only; mint a fresh linking code.
  - `GET  /v1/auth/whoami` — returns `Principal` for the current cookie/bearer
    or `401`.
- New static-file routes serving the Studio UI from `crates/actant-server/assets/`
  (owned by the Rust crate, not pulled from the npm package — keeps the
  workspace layout decoupled per CLAUDE.md binding rules) via `include_bytes!`:
  - `GET /studio` (and `/studio/*`) — the SPA shell.
  - `GET /link` and `GET /link/:code` — linking UI.
  - `GET /login` — password UI.
  - `GET /setup-password` — first-time password UI.
  - The HTML/CSS/JS in `assets/` is a **copy** of the matching files in
    `packages/actant-studio/ui/`. A `just sync-studio-assets` recipe diffs
    the two and fails CI when they drift; both packages share the same vanilla
    JS so the copy is mechanical.
- Extend `enforce_auth` so the chokepoint becomes:
  1. If `AppState.local_mode` and the request's `peer_addr.is_loopback()`,
     allow.
  2. Else if `Authorization: Bearer …` present, verify HS256 as today.
  3. Else if `Cookie: actantdb_session=…` present, look up
     `session_token`, check expiry/revocation, **enforce CSRF on mutating
     methods**, set `req.extensions().insert(Principal)`.
  4. Else 401.
- Add `AppState.local_mode: bool` set by `bootstrap` from the bind address
  (or env override).
- `bootstrap` extension: signature becomes `bootstrap(db_path, local_mode) ->
  (Router, AppState, Option<String /*display code*/>)`. It mints a link
  code whenever `local_mode == false` AND **no row exists in
  `workspace_owner` for the default workspace** — this covers both the
  empty-DB path (where `seed_if_empty` just created `ws_default`) and any
  restart where the DB has a workspace but the owner was never claimed
  (post-`seed_if_empty`, `workspace_owner` is empty). On mint, also
  `DELETE FROM link_code WHERE consumed_at IS NULL` to invalidate any prior
  unconsumed code. Returns `None` once an owner row exists. Printing happens
  in the binary; `lib.rs` stays silent.

### 7.4 Rust — `crates/actant-server/src/bin/server.rs`

- Detect bind address; pass `local_mode` to `bootstrap`.
- If `bootstrap` returns a freshly-minted link code, print the boot banner in
  §5.1 format.
- New subcommand `reset-password --workspace <id>` reads stdin for a new
  password, hashes, updates `workspace_owner`. (Convert `Args` to `clap`
  subcommand enum.)
- Refuse to start when bind is non-loopback and TLS is off, unless
  `ACTANTDB_INSECURE=1`.

### 7.5 TypeScript — `packages/actant-studio`

- New files (canonical copy; `crates/actant-server/assets/` mirrors these):
  `ui/link.{html,js}`, `ui/login.{html,js}`, `ui/setup-password.{html,js}`.
- `studio.js` `fetchJSON`: read CSRF from `<meta name="csrf-token">` injected
  by the server shell, attach `X-CSRF-Token` on mutating verbs. On `401`,
  redirect to `/login`.
- The Node Studio (`server.ts`) stays loopback-only for `--project X`
  developer workflows; it does **not** grow the linking flow (it has no
  ownership/session storage and no workspace concept — it talks to a single
  on-disk ledger keyed by project). The linking flow lives only in the Rust
  server. Documented in `packages/actant-studio/README.md`: Node Studio =
  ledger-on-disk dev tool, never non-loopback; remote = `actantdb-server`.

### 7.6 CLI — `packages/actant-studio/src/cli.ts`

- `actantdb studio` stays local-only and refuses `--bind 0.0.0.0` (error:
  "use `actantdb-server` for remote serving").
- `actantdb studio` opens the browser with `open` (new dep, peer-dep
  optional) when stdout is a TTY and `ACTANTDB_NO_BROWSER` is unset.
- New optional subcommand `actantdb serve` that shells out to the
  `actantdb-server` Rust binary if installed (looked up on PATH). Kept as a
  thin shim so the Node CLI stays Node-only — Rust binary is opt-in.

## 8. Security considerations summary

| Threat | Mitigation |
|---|---|
| Code stuffing | 60-bit code, 15-min TTL, per-IP rate limit, one-shot redemption, no timing oracle in response |
| Cookie theft via XSS | `HttpOnly`, no inline scripts, strict CSP `script-src 'self'` header |
| CSRF | `SameSite=Lax` + `X-CSRF-Token` on mutating verbs; bearer-token requests are CSRF-exempt by construction |
| Password brute force | argon2id; per-(workspace,ip) bucket; uniform 500 ms response after 5 failures |
| Reverse-proxy bypass of "loopback = trusted" | Detect forwarded headers without `ACTANTDB_TRUST_PROXY=1` and deny |
| Public bind without TLS | Refuse to start unless `ACTANTDB_INSECURE=1` |
| Session fixation | Token rotates on link, on login, every 7 days, on password change |
| Replay of leaked DB | Session token + link code stored as `sha256`, not plaintext |
| First-to-link race | `INSERT OR IGNORE` on `workspace_owner` + 409 to losers |

## 9. Verification gate

Add `tests/spec_auth_link_verification.rs` to `actant-server` covering:

1. Loopback bind: GET `/v1/healthz` and a gated route succeed without a
   cookie or bearer.
2. Non-loopback bind without an owner: bootstrap returns a code; POST
   `/v1/auth/link` with the wrong code returns 404; the right code returns
   200 + sets cookie; second redemption returns 409.
3. After link, mutating `POST /v1/command` without `X-CSRF-Token` returns
   403; with the right header returns 200.
4. After password set, fresh `/v1/auth/login` mints a new cookie; old
   pre-password cookie still works until expiry (no implicit revocation).
5. Public bind without TLS exits with the refusal error.
6. Bearer-token path (existing HS256) still works unchanged with auth on.
