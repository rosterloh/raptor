# Tasks

Three parts, each its own PR, in this order. Part 3 depends on part 2.

## 1. Certificate DDI auth

- [ ] 1.1 Confirm hawkBit's CN and issuer-hash header names and the `authentication.header.enabled` / `authentication.header.authority` tenant config keys against hawkBit source; adjust the design's defaults if they differ
- [ ] 1.2 Add `[ddi.certificate]` (`enabled`, `cn_header`, `issuer_hash_header`, `trusted_issuer_hashes`) to `DdiConfig` with defaults that leave behaviour unchanged; config parse test
- [ ] 1.3 Add `AuthKind::Certificate` and the certificate branch in `auth/ddi.rs::authenticate` (CN == controllerId, optional issuer-hash match over `-1`, `-2`, …), falling through to token auth on failure
- [ ] 1.4 Widen `api/ddi/artifacts.rs::check_entitled` to `Target | Certificate`; leave the auto-register refusal in `api/ddi/root.rs` on `Target` only
- [ ] 1.5 Startup warning when certificate auth is enabled and `bind` is not loopback
- [ ] 1.6 Report `authentication.header.enabled` / `authentication.header.authority` in `api/mgmt/system.rs`
- [ ] 1.7 Integration tests in `raptor/tests/ddi_auth.rs`: CN match, CN mismatch → 401, feature off ignores headers, trusted issuer accepted and rejected, fall-through to gateway token, auto-registration by certificate, foreign-module download → 404; system configs keys in `mgmt_system.rs`
- [ ] 1.8 Docs: certificate section in `guides/authentication.md` with an nginx example that verifies the client cert, strips client-supplied `X-Ssl-*` headers and sets them, plus the loopback-bind guidance; `[ddi.certificate]` in `reference/configuration.md`; Auth row in `intro/compatibility.md`
- [ ] 1.9 `CHANGELOG.md` Added bullet (#13)

## 2. Operator users and roles

- [ ] 2.1 Confirm hawkBit's insufficient-permission `403` body (`exceptionClass`, `errorCode`) and whether `/rest/v1/userinfo` exists upstream (match its fields if so)
- [ ] 2.2 Migration `m<date>_000001_users`: `users` table with `tenant` and named unique index `(tenant, username)`; SeaORM entity; migration test on SQLite (CI covers Postgres)
- [ ] 2.3 `Principal { username, role, source }`; session store keeps `{principal, expiry}` and gains `remove_user`
- [ ] 2.4 `mgmt_auth`: resolve cookie → Basic (config admin, then enabled local users) to a `Principal` extension; `login` accepts local users
- [ ] 2.5 Role gate in `mgmt_auth`: `read_only` allowed `GET`/`HEAD` and `POST /rest/v1/logout`, else `403`; `/rest/v1/users` admin-only
- [ ] 2.6 DTOs in `raptor-api-types` (`UserRest`, `UserCreate`, `UserUpdate`, `UserInfo`) with round-trip JSON tests; no password field on read DTOs
- [ ] 2.7 `api/mgmt/users.rs`: CRUD with paging/sort/FIQL (`username`, `role`, `enabled`, `source`), reserved config username → 409, self delete/disable/demote → 400, password on `oidc` user → 400, sessions revoked on delete/disable/role change; mapper in `mappers.rs`
- [ ] 2.8 `GET /rest/v1/userinfo`
- [ ] 2.9 Replace `cfg.mgmt.username` in `domain/rollout.rs` approve/deny with the request's `Principal`
- [ ] 2.10 Integration tests `raptor/tests/mgmt_users.rs`: CRUD and error paths, local login via Basic and session, read-only `403` on every non-GET route in the router (iterate the route table so new write routes are covered), revoked session → 401, attribution on rollout approval, empty table = unchanged behaviour
- [ ] 2.11 `raptorctl user list|create|set|delete` (TTY prompt or stdin for passwords, `-y` on delete)
- [ ] 2.12 Console: fetch `/rest/v1/userinfo` after login, hide write controls for `read_only`, show `403` as a permissions message, Users page for admins (follow `DESIGN.md`; regenerate and commit `raptor-ui/assets/tailwind.css`)
- [ ] 2.13 Docs: users and roles in `guides/authentication.md`, `/rest/v1/users` and `/rest/v1/userinfo` in `reference/management-api.md`, `raptorctl user` in `guides/cli.md`, compatibility Auth row, `web-console.md`
- [ ] 2.14 `CHANGELOG.md` Added bullet (#13)

## 3. OIDC

- [ ] 3.1 Ask for approval to add a JWT validation dependency (proposed `jsonwebtoken`); verify its latest version on crates.io before pinning
- [ ] 3.2 `[mgmt.oidc]` config (`issuer`, `client_id`, `client_secret`, `audience`, `scopes`, `role_claim`, `admin_values`, `read_only_values`, `username_claim`); startup requires `url` when present
- [ ] 3.3 Startup discovery and JWKS fetch via `reqwest`; fail `serve` on error; JWKS cache with refresh-on-unknown-`kid`, rate-limited
- [ ] 3.4 Bearer branch in `mgmt_auth`: signature, `iss`, `aud`, `exp`/`nbf` (60 s leeway); role mapping; JIT `users` upsert (`source = oidc`); disabled user → 401; no mapping → 403
- [ ] 3.5 `GET /rest/v1/oidc/login` (state, nonce, PKCE, 10-minute in-memory store) and `/rest/v1/oidc/callback` (verify state, code exchange, ID-token validation, session cookie, redirect to `/ui`); both outside the auth gate
- [ ] 3.6 Reject Basic/password login for `oidc` users
- [ ] 3.7 Integration tests `raptor/tests/mgmt_oidc.rs` against an in-process stub issuer (discovery, JWKS, token endpoint) signing test tokens: valid bearer, expired, wrong audience, wrong issuer, unknown `kid` then rotation, role mapping and no-match, JIT row, disabled user, forged `state`, full callback flow
- [ ] 3.8 `raptorctl`: `RAPTOR_TOKEN` sends a bearer token instead of Basic
- [ ] 3.9 Console: SSO button on the login page when OIDC is configured (expose that through a public unauthenticated flag, for example a `200`/`404` from `/rest/v1/oidc/login?probe`, decided in implementation)
- [ ] 3.10 Docs: OIDC section in `guides/authentication.md` with a Keycloak example, `[mgmt.oidc]` in `reference/configuration.md`, compatibility Auth row (OIDC, bearer)
- [ ] 3.11 `CHANGELOG.md` Added bullet (#13)

## 4. Verification (each part)

- [ ] 4.1 `mise run check`, plus the Postgres suite for the migration in part 2
- [ ] 4.2 Stock-client check: SWUpdate / rauc-hawkbit-updater / Zephyr token flows unchanged with each part enabled and disabled
- [ ] 4.3 `openspec validate auth-users-oidc-mtls --strict`; archive the change after part 3 merges
