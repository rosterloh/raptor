# Tasks

Three parts, each its own PR, in this order. Part 3 depends on part 2.

## 1. Certificate DDI auth

- [x] 1.1 Confirm hawkBit's header names and `authentication.header.*` keys against hawkBit 1.2.0 source (done 2026-11-12, see design §1)
- [x] 1.2 Add `[ddi.header_auth]` (`enabled`, `controller_id_header`, `authority_header`, `trusted_authorities`) to `DdiConfig` with defaults that leave behaviour unchanged; `trusted_proxies` (IP list, invalid entry → startup error), `auto_register` (default true); startup error when enabled with no trusted authority; config parse tests
- [x] 1.3 Add `AuthKind::Certificate` and the certificate branch in `auth/ddi.rs::authenticate` (controller-id header == controllerId; authority from the `,`/`;` list header or the `-0`, `-1`, … indexed headers, case-insensitive), falling through to token auth on failure; header auth attempted only when `trusted_proxies` is empty or contains the socket peer
- [x] 1.4 Widen `api/ddi/artifacts.rs::check_entitled` to `Target | Certificate`; in `api/ddi/root.rs` refuse auto-registration for `Certificate` when `auto_register = false`
- [x] 1.5 Startup warning when header auth is enabled, `trusted_proxies` is empty and `bind` is not loopback
- [x] 1.6 Report `authentication.header.enabled` / `authentication.header.authority` in `api/mgmt/system.rs`
- [x] 1.7 Integration tests in `raptor/tests/ddi_auth.rs`: controller id match and mismatch → 401, feature off ignores headers, list and indexed authority headers, untrusted authority, fall-through to gateway token, untrusted peer skips header auth, trusted peer allowed, auto-registration on and off, foreign-module download → 404; system configs keys in `mgmt_system.rs`
- [x] 1.8 Docs: certificate section in `guides/authentication.md` with an nginx example that verifies the client cert, strips client-supplied `X-Controller-Id` / `X-Authority*` headers and sets them from the certificate, plus the loopback-bind guidance; `[ddi.header_auth]` in `reference/configuration.md`; Auth row in `intro/compatibility.md`
- [x] 1.9 `CHANGELOG.md` Added bullet (#13)

## 2. Operator users and roles

- [x] 2.1 Confirm hawkBit's insufficient-permission `403` body and `/rest/v1/userinfo` shape (done 2026-11-12, see design §3 and §6)
- [x] 2.2 Migration `m<date>_000001_users`: `users` table with `tenant` and named unique index `(tenant, username)`; SeaORM entity; migration test on SQLite (CI covers Postgres)
- [x] 2.3 `Principal { username, role, source }`; session store keeps `{principal, expiry}` and gains `remove_user`
- [x] 2.4 `mgmt_auth`: resolve cookie → Basic (config admin, then enabled local users) to a `Principal` extension; `login` accepts local users
- [x] 2.5 `Role::permissions()` in hawkBit vocabulary as the single definition; role gate in `mgmt_auth`: `read_only` allowed `GET`/`HEAD` and `POST /rest/v1/logout`, else `403`; `/rest/v1/users` admin-only; unit test that the method gate and `permissions()` agree
- [x] 2.6 DTOs in `raptor-api-types` (`UserRest`, `UserCreate`, `UserUpdate`, `UserInfo` matching hawkBit's `MgmtUserInfo` plus `role`) with round-trip JSON tests; no password field on read DTOs; make `TargetRest.security_token` optional and skipped when absent
- [x] 2.7 `api/mgmt/users.rs`: CRUD with paging/sort/FIQL (`username`, `role`, `enabled`, `source`), reserved config username → 409, self delete/disable/demote → 400, password on `oidc` user → 400, sessions revoked on delete/disable/role change; mapper in `mappers.rs`
- [x] 2.8 `GET /rest/v1/userinfo` with hawkBit permission names per role; omit target `securityToken` for `read_only` in every target-returning mapper
- [x] 2.9 Replace `cfg.mgmt.username` in `domain/rollout.rs` approve/deny with the request's `Principal`
- [x] 2.10 Integration tests `raptor/tests/mgmt_users.rs`: CRUD and error paths, local login via Basic and session, read-only `403` on every non-GET route in the router (iterate the route table so new write routes are covered), revoked session → 401, attribution on rollout approval, empty table = unchanged behaviour
- [x] 2.11 `raptorctl user list|create|set|delete` (TTY prompt or stdin for passwords, `-y` on delete)
- [x] 2.12 Console: fetch `/rest/v1/userinfo` after login, hide write controls for `read_only`, show `403` as a permissions message, Users page for admins (follow `DESIGN.md`; regenerate and commit `raptor-ui/assets/tailwind.css`)
- [x] 2.13 Docs: users and roles in `guides/authentication.md`, `/rest/v1/users` and `/rest/v1/userinfo` in `reference/management-api.md`, `raptorctl user` in `guides/cli.md`, compatibility Auth row, `web-console.md`
- [x] 2.14 `CHANGELOG.md` Added bullet (#13)

## 3. OIDC

- [x] 3.1 Approval for `jsonwebtoken` (granted 2026-11-12; 11.1.0 is latest on crates.io, re-check at implementation time)
- [x] 3.2 `[mgmt.oidc]` config (`issuer`, `client_id`, `client_secret`, `audience`, `scopes`, `role_claim`, `admin_values`, `read_only_values`, `username_claim`); startup requires `url` when present
- [x] 3.3 Startup config validation (abort on error); discovery and JWKS fetch via `reqwest` that never aborts: `Unavailable` state (SSO `503`, bearer `401`), background retry with backoff 1 s → 5 min, `error` log per failure; JWKS cache with refresh-on-unknown-`kid`, rate-limited
- [x] 3.4 Bearer branch in `mgmt_auth`: signature, `iss`, `aud`, `exp`/`nbf` (60 s leeway); role mapping; JIT `users` upsert (`source = oidc`); disabled user → 401; no mapping → 403
- [x] 3.5 `GET /rest/v1/oidc/login` (state, nonce, PKCE, 10-minute in-memory store) and `/rest/v1/oidc/callback` (verify state, code exchange, ID-token validation, session cookie, redirect to `/ui`); both outside the auth gate
- [x] 3.6 Reject Basic/password login for `oidc` users
- [x] 3.7 Integration tests `raptor/tests/mgmt_oidc.rs` against an in-process stub issuer (discovery, JWKS, token endpoint) signing test tokens: valid bearer, expired, wrong audience, wrong issuer, unknown `kid` then rotation, issuer down at startup then recovering (DDI and config admin unaffected), role mapping and no-match, JIT row, disabled user, forged `state`, full callback flow
- [x] 3.8 `raptorctl`: `RAPTOR_TOKEN` sends a bearer token instead of Basic
- [x] 3.9 Console: SSO button on the login page when OIDC is configured (expose that through a public unauthenticated flag, for example a `200`/`404` from `/rest/v1/oidc/login?probe`, decided in implementation)
- [x] 3.10 Docs: OIDC section in `guides/authentication.md` with a Keycloak example, `[mgmt.oidc]` in `reference/configuration.md`, compatibility Auth row (OIDC, bearer)
- [x] 3.11 `CHANGELOG.md` Added bullet (#13)

## 4. Verification (each part)

- [x] 4.1 `mise run check`, plus the Postgres suite for the migration in part 2
- [ ] 4.2 Stock-client check: SWUpdate / rauc-hawkbit-updater / Zephyr token flows unchanged with each part enabled and disabled
- [ ] 4.3 `openspec validate auth-users-oidc-mtls --strict`; archive the change after part 3 merges
