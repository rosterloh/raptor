# Proposal: certificate DDI auth, operator users and OIDC

Tracks [#13](https://github.com/rosterloh/raptor/issues/13).

## Why

raptor's auth is deliberately minimal: devices use per-target or gateway tokens,
and every operator shares one Basic credential from `raptor.toml`. Production
fleets often provision devices with X.509 certificates instead of tokens, teams
need more than one operator login (some of them read-only), and organisations
with an identity provider want operators to sign in through it. hawkBit
supports all three. Its auth model has also settled in 1.x, so there is now a
stable design to match. And #12's tenant work is blocked until operators exist
as rows that a tenant can be attached to.

## What Changes

Three independently shippable parts, in order of value. Each is opt-in; a
`raptor.toml` from today keeps working with unchanged behaviour.

1. **Certificate DDI auth.** A TLS-terminating reverse proxy forwards the
   client certificate's CN, and optionally its issuer hash, in request headers
   (hawkBit's `X-Ssl-Client-*` pattern). When `[ddi.certificate]` is enabled,
   a CN equal to the request's `controllerId` authenticates as that device.
   The device can auto-register, and its downloads are limited to its own
   deployments, like a target token. Docs explain how to stop devices spoofing
   the headers. raptor warns at startup if it listens beyond loopback.
2. **Operator users and roles.** A new `users` table (with `tenant` from day
   one, and a composite `(tenant, username)` index) holds extra operator
   accounts, each with role `admin` or `read_only`. Read-only users can read
   everything and change nothing (`403`). The `[mgmt]` config account stays,
   always as an admin, so it still works as a break-glass login. Admins manage
   users through `/rest/v1/users` (a raptor extension), `raptorctl user` and a
   console page. Decisions and audit fields record the real user instead of
   the config username.
3. **OIDC.** With `[mgmt.oidc]` configured, the console offers single sign-on
   with the authorization-code flow and PKCE, ending in the normal session
   cookie. The Management API also accepts `Authorization: Bearer <JWT>` from
   that issuer, which is the path for automation and tooling (hawkBit 1.1
   added bearer auth). A configurable claim maps each identity to a role. The
   first login creates a matching `users` row, so OIDC users get the same
   attribution as local ones.

Not changing: DDI token auth, anonymous mode, the session cookie format, and
every existing JSON shape. `GET /rest/v1/session` still answers `204`/`401`.

## Capabilities

### New Capabilities

- `mgmt-users`: operator accounts beyond the config admin, their roles, how
  roles are enforced on the Management API, the user-administration API, and
  a user-info endpoint that tells clients who they are and their role.
- `oidc-login`: OIDC provider configuration, console single sign-on, bearer
  JWT validation and mapping an identity to a role.

### Modified Capabilities

- `ddi-auth`: adds certificate authentication via proxy headers, and widens
  auto-registration and download scoping to cover it.
- `mgmt-auth`: authenticates any operator (config admin, local user, OIDC),
  sessions carry the user and role, and decisions record the acting user.
- `system-info`: reports hawkBit's certificate-auth tenant config keys.
- `web-console`: SSO button, Users page for admins, and write controls hidden
  from read-only users.
- `raptorctl`: `user` commands, and bearer-token auth for scripting.

## Impact

- **Code**: `raptor/src/auth/` (new certificate branch, role check, bearer
  validation, session identity), new `api/mgmt/users.rs` and OIDC handlers,
  `config.rs` (`[ddi.certificate]`, `[mgmt.oidc]`), `domain/rollout.rs`
  (attribution), `raptor-api-types` (user DTOs), `raptor-ui` (login, users
  page, role-aware controls), `raptor-cli` (user commands, `RAPTOR_TOKEN`).
- **Schema**: one migration adding `users`. It must work on SQLite and
  Postgres.
- **Dependencies**: OIDC needs JWT signature validation (likely
  `jsonwebtoken`). It needs approval before it is added. `reqwest`, `rand`
  and `argon2` are already dependencies.
- **Docs**: `guides/authentication.md`, `reference/configuration.md`,
  `reference/management-api.md`, `intro/compatibility.md` (Auth table), a new
  reverse-proxy example for certificate auth, and `CHANGELOG.md`.
