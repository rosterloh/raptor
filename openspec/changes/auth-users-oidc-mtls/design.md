# Design

## Context

See `proposal.md` for the motivation. What exists today:

- **DDI auth** (`raptor/src/auth/ddi.rs`): one middleware checks the tenant
  segment, then returns an `AuthKind` (`Anonymous`, `Gateway`, `Target`) as a
  request extension. Two handlers branch on it.
  `api/ddi/root.rs` refuses to auto-register under `Target`.
  `api/ddi/artifacts.rs::check_entitled` limits downloads only for `Target`.
  `ddi.trusted_proxy_header` already sets the precedent for trusting a
  proxy-set header, but only for the recorded address.
- **Mgmt auth** (`auth/mgmt.rs`, `auth/session.rs`): one middleware accepts a
  session cookie or Basic credentials, checked against `cfg.mgmt` by
  `verify_creds`. Sessions are an in-memory `HashMap<token, expiry>` and carry
  no identity. Nothing downstream knows who the caller is: rollout approval
  hardcodes `cfg.mgmt.username` (`domain/rollout.rs:341`).
- **Schema rule** (#12): a new uniquely-named table carries `tenant` and a
  named composite unique index, never an inline `unique_key()`.
- `reqwest`, `rand`, `argon2`, `base64` and `url` are already dependencies.

## Goals / Non-Goals

**Goals:**
- Each of the three parts ships alone, behind its own config, in the order
  certificate → users → OIDC. Users must land before OIDC because OIDC
  identities are `users` rows.
- One place that answers "who is calling, with what role" for every Management
  API request, so later features (audit fields, tenant isolation) read it
  instead of re-deriving it.
- No change to any existing response shape or default behaviour.

**Non-Goals:**
- hawkBit's fine-grained permission matrix (`READ_TARGET`,
  `UPDATE_REPOSITORY`, …). Two roles cover the demand stated in #13. See the
  role decision below for how this can grow.
- raptor checking TLS itself. TLS stays at the proxy.
- Per-tenant isolation (#12). `users.tenant` is populated with the configured
  tenant and nothing filters on it yet.
- Multi-instance sessions. Sessions stay in memory, single process.
- An OIDC login flow inside `raptorctl`. Tooling gets bearer tokens from the
  provider by its own means and passes them with `RAPTOR_TOKEN`.
- Personal API tokens issued by raptor itself.

## Decisions

### 1. Certificate auth is another `AuthKind`, checked before tokens

Add `AuthKind::Certificate`. In `authenticate`, after the anonymous check,
certificate auth is tried first when enabled. A CN equal to `controllerId`
(and, if configured, a trusted issuer hash) wins. Anything else falls through
to the existing token branches unchanged. The two handler checks become
`matches!(auth, Target | Certificate)` for download scoping and
`auth == Target` stays as is for the auto-register refusal, so certificate
devices may self-register.

*Why fall through instead of rejecting?* hawkBit chains its auth filters
independently, and a proxy that forwards a gateway's own certificate must not
lock out that gateway's `GatewayToken` traffic. *Why allow auto-registration?*
Certificate fleets are provisioned at manufacture and never pre-created in
the server. Requiring a pre-created target would defeat the purpose, and the
certificate is already a per-device credential signed by a CA the operator
trusts.

Config:

```toml
[ddi.certificate]
enabled = true
cn_header = "X-Ssl-Client-Cn"              # default
issuer_hash_header = "X-Ssl-Issuer-Hash"   # default prefix; -1, -2, … read in turn
trusted_issuer_hashes = ["4b5a8e1f"]       # optional; empty = trust whatever the proxy verified
```

The header defaults follow hawkBit's reverse-proxy documentation as recalled
here; task 1.1 confirms them against hawkBit's source before they are frozen.
Alternative considered: raptor terminating TLS with `rustls` and reading the
peer certificate directly. Rejected for now: it would put TLS config, cert
reload and ALPN into raptor, against the "terminate TLS at a proxy" stance in
`guides/authentication.md`.

### 2. A `Principal` request extension for the Management API

`mgmt_auth` resolves every authenticated request to
`Principal { username, role, source }` and inserts it as an extension, the way
`ddi_auth` inserts `AuthKind`. Resolution order: session cookie → `Bearer`
(only when OIDC is configured) → `Basic` (config admin first, then local
users). The session store value becomes `{ principal, expiry }` instead of a
bare expiry.

Handlers that need attribution take `Extension<Principal>`. The first one is
rollout approve/deny, replacing `cfg.mgmt.username`.

### 3. Roles enforced by HTTP method, in the same middleware

`read_only` is allowed `GET`/`HEAD` on any route, plus `POST /rest/v1/logout`.
Everything else is `403`. `/rest/v1/users` writes and reads are admin-only.
The check lives in `mgmt_auth` right after resolution, so no handler can forget
it. A new route is protected by default.

*Alternative:* per-route permission annotations in hawkBit's vocabulary. That
gives finer control, but every route would need annotating and two roles
don't need it. If roles grow, the method rule becomes a `Role -> allowed(route,
method)` function in one place, and hawkBit's permission names are the
vocabulary to use, so policy translates between the two servers (comment on
#13).

The `403` body uses hawkBit's shape. Task 2.1 confirms hawkBit's
`exceptionClass`/`errorCode` for insufficient permissions.

### 4. `users` table; the config admin stays outside it

```
users(id, tenant, username, password_hash NULL, role, source, enabled,
      created_at, last_modified_at)
unique index users_tenant_username (tenant, username)
```

The `[mgmt]` account is not seeded into the table. That keeps today's
behaviour exact when the table is empty, keeps a break-glass admin that no API
call can delete or demote, and avoids two copies of one credential that could
disagree. Its username is reserved against user creation.

Revoking sessions on delete, disable or role change: the session store gains
`remove_user(username)`, called by those handlers. That is O(sessions), which
is fine at operator scale.

Local passwords use the same argon2id parameters as `raptor hash-password`.
They are set through the API over TLS, never echoed back, and `raptorctl`
reads them from a TTY prompt or stdin, never from argv.

### 5. OIDC: discovery at startup, PKCE code flow, JWKS-validated bearer

- **Startup**: fetch `<issuer>/.well-known/openid-configuration` and the JWKS
  with the existing `reqwest`. Failure stops `serve`, the same contract as the
  S3 `HEAD` check: a misconfigured IdP is found at deploy time, not at the
  first login.
- **Console SSO**: `/rest/v1/oidc/login` stores `{state, nonce, pkce_verifier}`
  in a short-lived (10 min) in-memory map and redirects. `/rest/v1/oidc/callback`
  checks `state`, exchanges the code, validates the ID token (signature, `iss`,
  `aud`, `exp`, `nonce`), maps the role, upserts the `users` row, creates a
  normal session and redirects to `/ui`. After that, the console treats it
  like a password login.
- **Bearer**: validate the JWT signature against the cached JWKS. An unknown
  `kid` triggers one refresh, at most once per minute. Then check `iss`,
  `aud ∋ audience`, and `exp`/`nbf` with 60 s leeway.
- **Role mapping**: `role_claim` (for example `groups`) may be a string or an
  array. `admin_values` wins over `read_only_values`. No match → `403`.
- **JIT rows**: the upsert keys on `username_claim` (default
  `preferred_username`). It lets admins see and disable OIDC users, and gives
  attribution a stable name. The role column on an `oidc` row is a cache of
  the last token. The token stays authoritative.

New dependency: JWT/JWS validation. `jsonwebtoken` is the likely choice
(widely used, RS/ES/PS algorithms, takes a JWK). Per the project's rules this
needs explicit approval before it is added, which is task 3.1. A full OIDC
client crate (`openidconnect`) was considered and rejected: it pulls a large
tree, and the flow above is small enough to own.

Config:

```toml
[mgmt.oidc]
issuer = "https://id.example.com/realms/ops"
client_id = "raptor"
client_secret = "…"            # prefer RAPTOR_MGMT__OIDC__CLIENT_SECRET
audience = "raptor"            # default: client_id
scopes = ["openid", "profile", "groups"]
role_claim = "groups"
admin_values = ["raptor-admins"]
read_only_values = ["raptor-viewers"]
username_claim = "preferred_username"
```

The redirect URI is derived from `url` (`<url>/rest/v1/oidc/callback`). OIDC
therefore requires `url` to be set, and startup fails without it when
`[mgmt.oidc]` is present.

### 6. Console and CLI learn the role from `/rest/v1/userinfo`

`/rest/v1/session` keeps its `204`/`401` contract, so the console's existing
probe is unchanged. A new `GET /rest/v1/userinfo` returns
`{username, role, tenant}`. Task 2.1 checks whether hawkBit has a same-named
endpoint, and if it does, its fields are kept and `role` is added. The console
calls it once after login and keeps the result in context.

## Risks / Trade-offs

- [Proxy headers are forgeable by anyone who reaches raptor directly] →
  certificate auth is off by default. The docs require binding to loopback or a
  private interface behind the proxy, and the proxy must strip incoming
  `X-Ssl-*` headers. A startup warning fires when `bind` is not loopback.
- [Fall-through could hide a misconfigured CN header (every request falls to
  tokens)] → log certificate rejections at `debug` with the reason, and count
  them under the `ddi` auth-failure metric only when the request finally fails.
- [Method-based roles are coarse] → accepted. A `GET` with side effects would
  leak writes to read-only users. No mgmt `GET` mutates today, and an
  integration test asserts a read-only user gets `403` on every non-GET route
  in the router, so a new write route cannot slip through.
- [In-memory sessions and OIDC state are lost on restart] → same as today.
  Users sign in again, and a login started before a restart fails `state`
  verification cleanly.
- [JIT `users` rows grow with every OIDC identity] → small. Admins can delete
  rows, and a deleted OIDC user is recreated only if their token still maps
  to a role.
- [The IdP being down at startup blocks raptor from starting, DDI included] →
  deliberate (fail fast), but it couples device service to the IdP's
  availability during restarts. Open question below.

## Migration Plan

- One migration, `mYYYYMMDD_NNNNNN_users`, creates `users` and its composite
  index. It is additive, needs no backfill, and runs on SQLite and Postgres.
- Rollback: revert the binary. The extra table is ignored by older releases,
  and nobody can log in as a table user until the new version is back.
- Upgrade needs no config change. Each part turns on by adding its section.

## Open Questions

- Should an unreachable IdP at startup degrade (log, disable OIDC, keep the
  config admin and local users working) rather than abort? The spec currently
  says abort. Switching later changes only startup behaviour, not routes or
  tasks.
- Exact hawkBit spellings to confirm in implementation (tasks 1.1, 2.1): the
  CN and issuer-hash header names, the `authentication.header.*` config keys,
  the insufficient-permission error body, and whether `/rest/v1/userinfo`
  exists upstream.
