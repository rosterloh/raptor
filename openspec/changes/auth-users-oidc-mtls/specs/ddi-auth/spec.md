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

### Requirement: Trusted proxies
When `[ddi.header_auth] trusted_proxies` lists IP addresses, header auth SHALL be attempted only for requests whose socket peer is one of them. Other requests SHALL skip header auth and fall through to token auth. An empty list (default) SHALL accept any peer. An entry that is not an IP address SHALL stop startup.

#### Scenario: Direct client forging headers
- **WHEN** `trusted_proxies = ["10.0.0.5"]` and a client at `10.0.0.9` sends a valid-looking `X-Controller-Id` and `X-Authority`
- **THEN** header auth is not attempted and, with no token, the response is `401`

#### Scenario: Request via the proxy
- **WHEN** the same headers arrive from peer `10.0.0.5`
- **THEN** header auth is attempted

### Requirement: Spoofing guard at startup
With header auth enabled, `trusted_proxies` empty and `bind` not a loopback address, raptor SHALL log a startup warning that the header-auth headers can be forged by any client able to reach it directly.

#### Scenario: Public bind
- **WHEN** header auth is enabled, `trusted_proxies` is empty and `bind = "0.0.0.0:8088"`
- **THEN** a warning naming the spoofing risk is logged at startup

#### Scenario: Proxies listed
- **WHEN** header auth is enabled with `trusted_proxies` set and a public `bind`
- **THEN** no spoofing warning is logged

## MODIFIED Requirements

### Requirement: Only shared-credential requests may auto-register
An unknown `controllerId` SHALL be registered on its first poll only when the request authenticated with the gateway token, in anonymous mode, or by certificate while `[ddi.header_auth] auto_register` is true (the default). A target-token request can never create a target.

#### Scenario: Gateway registration
- **WHEN** an unknown `dev-9` polls with a valid gateway token
- **THEN** `dev-9` is created with status `registered`

#### Scenario: Certificate registration
- **WHEN** header auth is on and an unknown `dev-10` polls with `X-Controller-Id: dev-10` and a trusted authority
- **THEN** `dev-10` is created with status `registered`

#### Scenario: Certificate registration disabled
- **WHEN** `auto_register = false` and an unknown `dev-11` polls with valid header-auth headers
- **THEN** the response is `401` and no target is created

### Requirement: Target-token downloads are scoped to the target's deployments
A target authenticated by its own token or by certificate SHALL list and download only artifacts of software modules in distribution sets it has, or had, an action for. Any other module SHALL return `404`. Gateway and anonymous requests are not scoped this way.

#### Scenario: Foreign module
- **WHEN** `dev-1` using its target token requests artifacts of a module that is in no set ever assigned to it
- **THEN** the response is `404`

#### Scenario: Foreign module by certificate
- **WHEN** `dev-1` authenticated by certificate requests artifacts of a module never assigned to it
- **THEN** the response is `404`
