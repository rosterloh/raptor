# mgmt-auth Specification

## Purpose

How operators authenticate to the Management API (`/rest/v1/...`) and the web
console: HTTP Basic against one configured admin account, or a browser session
cookie obtained by logging in.

## Requirements

### Requirement: Single configured operator account
The Management API SHALL authenticate against exactly one account, `[mgmt] username` and `[mgmt] password_hash` (an argon2id PHC string). Passwords SHALL never be stored or accepted in plaintext config. An unparseable hash SHALL authenticate nobody.

#### Scenario: Correct credentials
- **WHEN** a request sends Basic credentials matching `username` and the password behind `password_hash`
- **THEN** the request is authenticated

#### Scenario: Wrong password
- **WHEN** the username matches but the password does not
- **THEN** the response is `401`

### Requirement: Password hashing helper
`raptor hash-password` SHALL read a password from stdin and print its argon2id PHC hash for `password_hash`.

#### Scenario: Hash a password
- **WHEN** an operator runs `printf 'pw\n' | raptor hash-password`
- **THEN** a `$argon2id$...` string is printed

### Requirement: Every Management API route requires authentication
All `/rest/v1/*` routes except `login` and `logout` SHALL require a valid session cookie or valid Basic credentials. `/health` SHALL stay unauthenticated. Each failure SHALL be counted as a `mgmt` auth failure.

#### Scenario: No credentials
- **WHEN** `GET /rest/v1/targets` is sent without credentials
- **THEN** the response is `401` with the hawkBit `hawkbit.server.error.unauthorized` error body

#### Scenario: Health probe
- **WHEN** `GET /health` is sent without credentials
- **THEN** the response is `200` with body `ok`

### Requirement: Basic challenge, quiet for the console
A `401` from the Management API SHALL carry `WWW-Authenticate: Basic realm="raptor"`, except when the request has an `X-Requested-With` header. Those quiet failures SHALL not be logged as warnings.

#### Scenario: Console session probe
- **WHEN** the console calls `GET /rest/v1/session` with `X-Requested-With` and no valid session
- **THEN** the response is `401` without `WWW-Authenticate`

### Requirement: Session login and logout
`POST /rest/v1/login` with `{"username","password"}` SHALL, on valid credentials, return `204` and set an `HttpOnly; SameSite=Strict; Path=/` `raptor_session` cookie holding a random 256-bit token, adding `Secure` when `X-Forwarded-Proto` is `https`. `POST /rest/v1/logout` SHALL drop the session and expire the cookie.

#### Scenario: Bad login
- **WHEN** `POST /rest/v1/login` is sent with a wrong password
- **THEN** the response is `401` with a `BadCredentialsException` error body and no cookie

#### Scenario: Logout
- **WHEN** a logged-in browser posts to `/rest/v1/logout`
- **THEN** the cookie is cleared and the old token no longer authenticates

### Requirement: Session lifetime
Sessions SHALL expire after 24 hours without use, and each authenticated request SHALL extend that window. Sessions SHALL be held in server memory only, so a restart logs everyone out.

#### Scenario: Idle expiry
- **WHEN** a session token is presented more than 24 hours after its last use
- **THEN** it is rejected and removed

### Requirement: Session probe
`GET /rest/v1/session` SHALL return `204` for an authenticated request and `401` otherwise.

#### Scenario: Valid cookie
- **WHEN** the console probes with a live session cookie
- **THEN** the response is `204`

### Requirement: Decisions are attributed to the operator account
Where the API records who made a decision (for example a rollout's `approveDecidedBy`), it SHALL record the configured `username`.

#### Scenario: Rollout approval
- **WHEN** an operator approves a rollout
- **THEN** `approveDecidedBy` is the configured `username`
