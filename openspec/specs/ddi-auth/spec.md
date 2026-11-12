# ddi-auth Specification

## Purpose

How devices prove who they are on the DDI API (`/{tenant}/controller/v1/...`):
target tokens, a shared gateway token, or anonymous mode. The method a request
used also decides whether unknown devices may self-register and which artifacts
a device may download.

## Requirements

### Requirement: Tenant segment is checked before authentication
Every DDI request SHALL be rejected with `404` unless its `{tenant}` path segment equals the configured `tenant` (default `DEFAULT`), compared case-insensitively. This check SHALL run before any credential is examined.

#### Scenario: Lowercase default tenant
- **WHEN** a device requests `/default/controller/v1/dev-1` and `tenant` is unset
- **THEN** the request proceeds to authentication

#### Scenario: Unknown tenant
- **WHEN** a device requests `/acme/controller/v1/dev-1` and `tenant` is `DEFAULT`
- **THEN** the response is `404`, whatever credentials were sent

### Requirement: Target token authentication
A request carrying `Authorization: TargetToken <token>` SHALL authenticate only when a target with the path's `controllerId` exists and its security token equals `<token>`. Otherwise the response SHALL be `401`.

#### Scenario: Matching token
- **WHEN** `dev-1` exists with token `abc` and polls with `TargetToken abc`
- **THEN** the request is authenticated as that target

#### Scenario: Unknown controller
- **WHEN** a request uses `TargetToken abc` for a `controllerId` that does not exist
- **THEN** the response is `401` and no target is created

### Requirement: Gateway token authentication
A request carrying `Authorization: GatewayToken <token>` SHALL authenticate when `[ddi] gateway_token` is set and equals `<token>`, for any `controllerId`. Otherwise the response SHALL be `401`.

#### Scenario: Gateway token unset
- **WHEN** `gateway_token` is not configured and a request sends `GatewayToken x`
- **THEN** the response is `401`

### Requirement: Anonymous mode
When `[ddi] anonymous` is true, every DDI request SHALL be accepted without credentials. It SHALL be off by default. It is a raptor extension; hawkBit removed anonymous controllers in 0.8.

#### Scenario: Anonymous poll
- **WHEN** `anonymous = true` and a request carries no `Authorization` header
- **THEN** the request is authenticated anonymously

### Requirement: Missing or unrecognised credentials are rejected
With anonymous mode off, a request with no `Authorization` header, or one using any other scheme, SHALL get `401`. Each failure SHALL be logged at warn level with the request path and counted as a `ddi` auth failure.

#### Scenario: Basic credentials on DDI
- **WHEN** a device sends `Authorization: Basic ...` to a DDI endpoint
- **THEN** the response is `401`

### Requirement: Only shared-credential requests may auto-register
An unknown `controllerId` SHALL be registered on its first poll only when the request authenticated with the gateway token or in anonymous mode. A target-token request can never create a target.

#### Scenario: Gateway registration
- **WHEN** an unknown `dev-9` polls with a valid gateway token
- **THEN** `dev-9` is created with status `registered`

### Requirement: Target-token downloads are scoped to the target's deployments
A target authenticated by its own token SHALL list and download only artifacts of software modules in distribution sets it has, or had, an action for. Any other module SHALL return `404`. Gateway and anonymous requests are not scoped this way.

#### Scenario: Foreign module
- **WHEN** `dev-1` using its target token requests artifacts of a module that is in no set ever assigned to it
- **THEN** the response is `404`
