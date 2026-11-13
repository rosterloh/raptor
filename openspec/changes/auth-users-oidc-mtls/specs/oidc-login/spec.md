# Spec Delta

## Purpose

Operator sign-in through an external OpenID Connect provider: console single
sign-on, bearer-token access for tooling, and mapping identities to raptor roles.

## ADDED Requirements

### Requirement: Opt-in provider configuration
OIDC SHALL be enabled only by a `[mgmt.oidc]` section with `issuer`, `client_id`, `client_secret`, `role_claim` and the claim values that grant `admin` and `read_only`. Invalid configuration (missing `url`, an unparseable issuer URL, no role values) SHALL stop `raptor serve` at startup.

#### Scenario: Missing base URL
- **WHEN** `[mgmt.oidc]` is set and `url` is not
- **THEN** `raptor serve` exits with an error before listening

#### Scenario: Section absent
- **WHEN** there is no `[mgmt.oidc]` section
- **THEN** no OIDC routes respond and bearer tokens are rejected

### Requirement: Provider outages never stop the server
raptor SHALL start, and keep serving the DDI API and non-OIDC operators, while the provider's discovery document or keys cannot be fetched. Until they are, SSO login SHALL return `503` and bearer tokens `401`. raptor SHALL log the failure and retry with backoff.

#### Scenario: Issuer down at startup
- **WHEN** raptor starts while the issuer is unreachable
- **THEN** devices poll normally, the config admin can log in, and `/rest/v1/oidc/login` returns `503`

#### Scenario: Issuer recovers
- **WHEN** the issuer becomes reachable after startup
- **THEN** a later retry loads its keys and SSO and bearer auth start working without a restart

### Requirement: Console single sign-on
`GET /rest/v1/oidc/login` SHALL redirect to the provider using the authorization-code flow with PKCE, a random `state` and a `nonce`. The callback SHALL verify `state`, exchange the code, validate the ID token, then set the normal session cookie and redirect to the console.

#### Scenario: Forged callback
- **WHEN** the callback arrives with a `state` raptor did not issue
- **THEN** no session is created and the response is `401`

### Requirement: Bearer token access
The Management API SHALL accept `Authorization: Bearer <JWT>` signed by the configured issuer, with `iss` equal to the issuer, `aud` containing the configured audience (default `client_id`), and `exp`/`nbf` valid within 60 s of clock skew. Any other token SHALL be `401`.

#### Scenario: Expired token
- **WHEN** a script sends a bearer token whose `exp` has passed
- **THEN** the response is `401`

#### Scenario: Wrong audience
- **WHEN** a token issued for another client is presented
- **THEN** the response is `401`

### Requirement: Key rotation
A token signed with an unknown key id SHALL trigger one refresh of the provider's keys before rejecting it. Refreshes SHALL be rate-limited.

#### Scenario: Provider rotates keys
- **WHEN** the provider starts signing with a new key
- **THEN** the first such token is accepted after a key refresh

### Requirement: Role mapping
An identity's role SHALL come from `role_claim`: `admin` if the claim contains an admin value, else `read_only` if it contains a read-only value. An identity with neither SHALL be refused: `403` at login or on a bearer request.

#### Scenario: No matching group
- **WHEN** a user whose `groups` claim contains only `sales` signs in
- **THEN** sign-in fails with `403` and no session is created

### Requirement: Just-in-time user records
The first successful OIDC sign-in or bearer request SHALL create a `users` row with source `oidc`, keyed by the configured username claim (default `preferred_username`). The role SHALL be re-evaluated from the token on every login or request. An admin-disabled OIDC user SHALL be refused even with a valid token.

#### Scenario: Disabled OIDC user
- **WHEN** an admin disables OIDC user `carol` and she presents a valid token
- **THEN** the response is `401`

### Requirement: No local password for OIDC users
A `users` row with source `oidc` SHALL never authenticate through Basic or `POST /rest/v1/login`.

#### Scenario: Password login attempt
- **WHEN** someone posts `carol`'s username and any password to `/rest/v1/login`
- **THEN** the response is `401`
