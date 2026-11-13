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
header auth is tried first when enabled. It succeeds when the controller-id
header equals `controllerId` and an authority header names a trusted
authority. Anything else falls through to the existing token branches
unchanged. The two handler checks become `matches!(auth, Target | Certificate)`
for download scoping. Certificate devices may self-register, as upstream,
unless `auto_register = false`. That switch is for operators whose trusted CA
also signs certificates for things that are not devices.

This mirrors hawkBit 1.2's `SecurityHeaderAuthenticator`, verified against
source (`hawkbit-ddi-security`, tag 1.2.0):

- Header names default to `X-Controller-Id` and `X-Authority`
  (`hawkbit.server.ddi.security.rp.controllerIdHeader` / `authorityHeader`).
- The authority header is either one `,`/`;`-separated value, or, when that
  header is absent, `X-Authority-0`, `-1`, … read until one is missing
  (starting at 0, or at 1 if `-0` is absent). Matching is case-insensitive
  and trimmed.
- A trusted authority is mandatory. Upstream, an empty authority config
  matches nothing, which silently disables the feature. raptor makes that a
  startup error instead.
- Each DDI auth filter runs independently and only sets the authentication on
  success, which is where the fall-through comes from.
- Every successful DDI authenticator yields the same controller principal, and
  `findOrRegisterTargetIfItDoesNotExist` runs for all of them, so header-auth
  devices auto-register upstream as well.
- Upstream also has `rp.trustedIPs`, a source-IP allowlist (exact
  addresses) applied to every DDI auth filter. raptor adopts it as
  `trusted_proxies`, but scoped to header auth only. Token auth never trusted
  proxy headers, so gating it by peer IP would only break direct device
  connections. Requests from other peers skip header auth and fall through to
  tokens. The comparison uses the socket peer, never `trusted_proxy_header`,
  because the proxy is the peer by definition.

Config:

```toml
[ddi.header_auth]
enabled = true
controller_id_header = "X-Controller-Id"   # default
authority_header = "X-Authority"           # default
trusted_authorities = ["4b5a8e1f"]         # required when enabled
trusted_proxies = ["10.0.0.5"]             # optional; empty = any peer (startup warning if bind is public)
auto_register = true                       # default
```

Reported as `authentication.header.enabled` and
`authentication.header.authority` (the list joined with `,`). The same key
names appear in hawkBit's `TenantConfigurationProperties` and its UI.

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

### 3. Roles enforced by HTTP method; permissions defined in one place

`read_only` is allowed `GET`/`HEAD` on any route, plus `POST /rest/v1/logout`.
Everything else is `403`. `/rest/v1/users` writes and reads are admin-only.
The check lives in `mgmt_auth` right after resolution, so no handler can forget
it. A new route is protected by default.

Each role's hawkBit permission set (`Role::permissions()`) is also defined in
one function. `/rest/v1/userinfo` reports it, and the method gate is checked
against it in a unit test, so what raptor reports and what it enforces cannot
drift apart.

*Alternative chosen against for now:* per-route permission annotations in
hawkBit's vocabulary (`READ_TARGET`, `APPROVE_ROLLOUT`, …). That allows more
roles, but every route (about 100) needs labelling. The trigger to move to it
is a third role. The likeliest is an approver, since with one write role the
rollout approval gate is self-approval. At that point the gate becomes
`principal.permissions().contains(route.required)` and `permissions()` is
already there.

The `403` body is hawkBit's `SP_INSUFFICIENT_PERMISSION`:
`exceptionClass` `org.eclipse.hawkbit.repository.exception.InsufficientPermissionException`,
`errorCode` `hawkbit.server.error.insufficientPermission` (verified in
`hawkbit-core` `SpServerError` and `RestConfiguration`, 1.2.0).

Read-only users also never see target `securityToken`s. hawkBit gates them
behind a separate `READ_TARGET_SECURITY_TOKEN` permission, because a token is
enough to impersonate a device. `TargetRest.security_token` becomes
`Option<String>` with `skip_serializing_if`, filled only for admins.

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

- **Startup**: validate the config (issuer URL parses, `url` set, role values
  present), and abort on error. Then fetch `<issuer>/.well-known/openid-configuration`
  and the JWKS with the existing `reqwest`. A *fetch* failure does not abort:
  OIDC state is `Unavailable`, SSO returns `503`, bearer tokens get `401`,
  and a background task retries with exponential backoff (1 s → 5 min) until
  it loads. Unlike the S3 `HEAD` check, which aborts, devices never need the
  IdP, so an IdP outage during a restart must not take the DDI API down with
  it.
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

New dependency: `jsonwebtoken` 11.1.0 (latest on crates.io, 2026-09-16) for
JWS validation against JWKs. Approved 2026-11-12. A full OIDC client crate
(`openidconnect`) was considered and rejected: it pulls a large tree, and the
flow above is small enough to own.

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
probe is unchanged. hawkBit already has `GET /rest/v1/userinfo` returning
`MgmtUserInfo {tenant, username, permissions[]}` (verified, 1.2.0). raptor
implements that shape, with `permissions` in hawkBit's vocabulary
(`READ_TARGET`, `CREATE_ROLLOUT`, `APPROVE_ROLLOUT`, …), and adds a `role`
field as an extension. `admin` maps to the full permission set and
`read_only` to the `READ_*` set without `READ_TARGET_SECURITY_TOKEN` and
`READ_GATEWAY_SECURITY_TOKEN`. hawkBit tooling that inspects permissions
therefore works unchanged, and finer roles can later be expressed as other
permission sets. The console calls it once after login and keeps the result
in context.

## Risks / Trade-offs

- [Proxy headers are forgeable by anyone who reaches raptor directly] →
  header auth is off by default and needs a trusted authority. The docs require binding to loopback or a
  private interface behind the proxy, and the proxy must strip incoming
  `X-Controller-Id` / `X-Authority*` headers. A startup warning fires when `bind` is not loopback.
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
- [An IdP that is down at startup leaves SSO unavailable] → raptor still
  starts. The failure is logged at `error` on every retry, so a provider that
  is wrong rather than down (wrong issuer URL that resolves, for example)
  shows up in logs, not as a silent hang. The config admin and local users
  keep working.
- [With one write role, rollout approval is self-approval] → accepted for
  now. It is the documented trigger to move to per-route permissions and add
  an approver role.

## Migration Plan

- One migration, `mYYYYMMDD_NNNNNN_users`, creates `users` and its composite
  index. It is additive, needs no backfill, and runs on SQLite and Postgres.
- Rollback: revert the binary. The extra table is ignored by older releases,
  and nobody can log in as a table user until the new version is back.
- Upgrade needs no config change. Each part turns on by adding its section.
