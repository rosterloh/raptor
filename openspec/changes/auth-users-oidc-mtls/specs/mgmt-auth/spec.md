# Spec Delta

## RENAMED Requirements

- FROM: `### Requirement: Single configured operator account`
- TO: `### Requirement: Operator accounts`

## MODIFIED Requirements

### Requirement: Operator accounts
The Management API SHALL authenticate the `[mgmt]` config account (`username` plus an argon2id `password_hash`), and also enabled local users and OIDC users (see `mgmt-users` and `oidc-login`). Passwords SHALL never be stored or accepted in plaintext config. An unparseable hash SHALL authenticate nobody.

#### Scenario: Correct credentials
- **WHEN** a request sends Basic credentials matching `username` and the password behind `password_hash`
- **THEN** the request is authenticated

#### Scenario: Wrong password
- **WHEN** the username matches but the password does not
- **THEN** the response is `401`

#### Scenario: Local user
- **WHEN** a request sends Basic credentials of an enabled local user
- **THEN** the request is authenticated as that user with their role

### Requirement: Every Management API route requires authentication
All `/rest/v1/*` routes except `login`, `logout` and the OIDC login/callback routes SHALL require a valid session cookie, valid Basic credentials, or (with OIDC configured) a valid bearer token. `/health` SHALL stay unauthenticated. Each failure SHALL be counted as a `mgmt` auth failure.

#### Scenario: No credentials
- **WHEN** `GET /rest/v1/targets` is sent without credentials
- **THEN** the response is `401` with the hawkBit `hawkbit.server.error.unauthorized` error body

#### Scenario: Health probe
- **WHEN** `GET /health` is sent without credentials
- **THEN** the response is `200` with body `ok`

#### Scenario: Bearer without OIDC
- **WHEN** OIDC is not configured and a request sends `Authorization: Bearer x`
- **THEN** the response is `401`

### Requirement: Session login and logout
`POST /rest/v1/login` with `{"username","password"}` SHALL, on valid credentials of any password-based operator, return `204` and set an `HttpOnly; SameSite=Strict; Path=/` `raptor_session` cookie holding a random 256-bit token, adding `Secure` when `X-Forwarded-Proto` is `https`. The session SHALL record the user and role. `POST /rest/v1/logout` SHALL drop the session and expire the cookie.

#### Scenario: Bad login
- **WHEN** `POST /rest/v1/login` is sent with a wrong password
- **THEN** the response is `401` with a `BadCredentialsException` error body and no cookie

#### Scenario: Logout
- **WHEN** a logged-in browser posts to `/rest/v1/logout`
- **THEN** the cookie is cleared and the old token no longer authenticates

#### Scenario: Read-only session
- **WHEN** a `read_only` user logs in and then sends a write request with the cookie
- **THEN** the write is `403`

### Requirement: Decisions are attributed to the operator account
Where the API records who made a decision (for example a rollout's `approveDecidedBy`), it SHALL record the username of the operator who made the request: the config account, a local user, or an OIDC user.

#### Scenario: Rollout approval
- **WHEN** local user `alice` approves a rollout
- **THEN** `approveDecidedBy` is `alice`

## ADDED Requirements

### Requirement: Disabled or deleted users lose access immediately
Disabling or deleting a user, or changing their role, SHALL end their sessions, so the next request with an old cookie is `401`.

#### Scenario: Deleted mid-session
- **WHEN** an admin deletes user `bob` while `bob` has an open console session
- **THEN** `bob`'s next request is `401`
