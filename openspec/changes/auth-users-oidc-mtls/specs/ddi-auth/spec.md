# Spec Delta

## ADDED Requirements

### Requirement: Certificate authentication via proxy headers
When `[ddi.certificate] enabled` is true, a request whose CN header (`cn_header`, default `X-Ssl-Client-Cn`) equals the path's `controllerId` SHALL authenticate as that device. It SHALL be off by default, leaving behaviour unchanged.

#### Scenario: Matching CN
- **WHEN** certificate auth is on and `dev-1` polls with `X-Ssl-Client-Cn: dev-1` and no `Authorization` header
- **THEN** the request is authenticated as `dev-1` by certificate

#### Scenario: Feature off
- **WHEN** certificate auth is off and a request carries only `X-Ssl-Client-Cn: dev-1`
- **THEN** the header is ignored and the response is `401`

### Requirement: Trusted issuer check
When `[ddi.certificate] trusted_issuer_hashes` is non-empty, certificate auth SHALL also require one of the issuer-hash headers (`issuer_hash_header`, default `X-Ssl-Issuer-Hash-1`, then `-2`, `-3`… while present) to match a listed hash, compared case-insensitively. An empty list SHALL skip the check.

#### Scenario: Unknown issuer
- **WHEN** the CN matches but no issuer-hash header matches a trusted hash
- **THEN** certificate auth fails

### Requirement: Certificate failure falls through to tokens
A request whose certificate headers are missing or do not authenticate SHALL still be tried against target-token and gateway-token auth, and SHALL get `401` only when none succeeds.

#### Scenario: Gateway behind certificate proxy
- **WHEN** certificate auth is on and a request has CN `gw-1` for controller `dev-7` plus a valid `GatewayToken`
- **THEN** it is authenticated by the gateway token

### Requirement: Spoofing guard at startup
With certificate auth enabled and `bind` not a loopback address, raptor SHALL log a startup warning that the certificate headers can be forged by any client able to reach it directly.

#### Scenario: Public bind
- **WHEN** certificate auth is enabled and `bind = "0.0.0.0:8088"`
- **THEN** a warning naming the spoofing risk is logged at startup

## MODIFIED Requirements

### Requirement: Only shared-credential requests may auto-register
An unknown `controllerId` SHALL be registered on its first poll only when the request authenticated with the gateway token, by certificate, or in anonymous mode. A target-token request can never create a target.

#### Scenario: Gateway registration
- **WHEN** an unknown `dev-9` polls with a valid gateway token
- **THEN** `dev-9` is created with status `registered`

#### Scenario: Certificate registration
- **WHEN** certificate auth is on and an unknown `dev-10` polls with CN `dev-10`
- **THEN** `dev-10` is created with status `registered`

### Requirement: Target-token downloads are scoped to the target's deployments
A target authenticated by its own token or by certificate SHALL list and download only artifacts of software modules in distribution sets it has, or had, an action for. Any other module SHALL return `404`. Gateway and anonymous requests are not scoped this way.

#### Scenario: Foreign module
- **WHEN** `dev-1` using its target token requests artifacts of a module that is in no set ever assigned to it
- **THEN** the response is `404`

#### Scenario: Foreign module by certificate
- **WHEN** `dev-1` authenticated by certificate requests artifacts of a module never assigned to it
- **THEN** the response is `404`
