# Spec Delta

## ADDED Requirements

### Requirement: Certificate authentication via proxy headers
When `[ddi.header_auth] enabled` is true, a request whose controller-id header (`controller_id_header`, default `X-Controller-Id`) equals the path's `controllerId`, and whose authority header names a trusted authority, SHALL authenticate as that device. It SHALL be off by default, leaving behaviour unchanged.

#### Scenario: Matching controller id and authority
- **WHEN** header auth is on with `trusted_authorities = ["ca-a"]` and `dev-1` polls with `X-Controller-Id: dev-1` and `X-Authority: ca-a`
- **THEN** the request is authenticated as `dev-1` by certificate

#### Scenario: Feature off
- **WHEN** header auth is off and a request carries only `X-Controller-Id: dev-1`
- **THEN** the headers are ignored and the response is `401`

### Requirement: Trusted authority check
Header auth SHALL require at least one entry in `trusted_authorities`; enabling it with an empty list SHALL stop `raptor serve` at startup. The authority SHALL be read from `authority_header` (default `X-Authority`) as a `,`/`;`-separated list, or, when absent, from `<authority_header>-0`, `-1`, … until a header is missing. Any match, compared case-insensitively after trimming, SHALL be accepted.

#### Scenario: Indexed authority headers
- **WHEN** a request has no `X-Authority` but `X-Authority-1: CA-A` and `ca-a` is trusted
- **THEN** the authority check passes

#### Scenario: Untrusted authority
- **WHEN** the controller id matches but no authority header names a trusted authority
- **THEN** header auth fails

#### Scenario: Empty list
- **WHEN** `enabled = true` and `trusted_authorities` is empty
- **THEN** `raptor serve` exits with an error before listening

### Requirement: Certificate failure falls through to tokens
A request whose header-auth headers are missing or do not authenticate SHALL still be tried against target-token and gateway-token auth, and SHALL get `401` only when none succeeds.

#### Scenario: Gateway behind certificate proxy
- **WHEN** header auth is on and a request has `X-Controller-Id: gw-1` for controller `dev-7` plus a valid `GatewayToken`
- **THEN** it is authenticated by the gateway token

### Requirement: Spoofing guard at startup
With header auth enabled and `bind` not a loopback address, raptor SHALL log a startup warning that the header-auth headers can be forged by any client able to reach it directly.

#### Scenario: Public bind
- **WHEN** header auth is enabled and `bind = "0.0.0.0:8088"`
- **THEN** a warning naming the spoofing risk is logged at startup

## MODIFIED Requirements

### Requirement: Only shared-credential requests may auto-register
An unknown `controllerId` SHALL be registered on its first poll only when the request authenticated with the gateway token, by certificate, or in anonymous mode. A target-token request can never create a target.

#### Scenario: Gateway registration
- **WHEN** an unknown `dev-9` polls with a valid gateway token
- **THEN** `dev-9` is created with status `registered`

#### Scenario: Certificate registration
- **WHEN** header auth is on and an unknown `dev-10` polls with `X-Controller-Id: dev-10` and a trusted authority
- **THEN** `dev-10` is created with status `registered`

### Requirement: Target-token downloads are scoped to the target's deployments
A target authenticated by its own token or by certificate SHALL list and download only artifacts of software modules in distribution sets it has, or had, an action for. Any other module SHALL return `404`. Gateway and anonymous requests are not scoped this way.

#### Scenario: Foreign module
- **WHEN** `dev-1` using its target token requests artifacts of a module that is in no set ever assigned to it
- **THEN** the response is `404`

#### Scenario: Foreign module by certificate
- **WHEN** `dev-1` authenticated by certificate requests artifacts of a module never assigned to it
- **THEN** the response is `404`
