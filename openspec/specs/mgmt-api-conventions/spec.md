# mgmt-api-conventions Specification

## Purpose

The cross-cutting Management API contract every resource follows: hawkBit
paging, sorting, FIQL filtering, error bodies and status codes. hawkBit
compatibility of this wire format is the contract.

## Requirements

### Requirement: Paged list envelope
Every list endpoint SHALL accept `offset` and `limit` and return hawkBit's envelope `{"content": [...], "total": N, "size": n}`.

#### Scenario: Second page
- **WHEN** a client lists targets with `offset=50&limit=50` and 120 exist
- **THEN** `total` is 120, `size` is 50, and `content` holds targets 51–100

### Requirement: Sorting
List endpoints SHALL accept `sort=<field>:ASC|DESC` over the resource's documented fields.

#### Scenario: Sort by version
- **WHEN** a client lists distribution sets with `sort=version:DESC`
- **THEN** results are ordered by version, descending

### Requirement: FIQL filtering
List endpoints SHALL accept `q=<FIQL>` with `==`, `!=`, `=lt=`, `=le=`, `=gt=`, `=ge=`, `=in=`, `=out=`, `;` (AND, binding tighter) and `,` (OR), plus parentheses. `*` SHALL be a wildcard, while `%` and `_` match themselves.

#### Scenario: Wildcard
- **WHEN** a client queries `controllerId==dev-*`
- **THEN** every target whose id starts with `dev-` matches

### Requirement: Case-insensitive text comparison
Text comparisons (`==`, `!=`, `=in=`, `=out=`) SHALL ignore case on both SQLite and Postgres.

#### Scenario: Mixed case
- **WHEN** a client queries `name==Gateway*`
- **THEN** a target named `gateway-1` matches

### Requirement: Typed and validated fields
Each resource SHALL expose a fixed FIQL field map. An unknown field, an ill-typed value (non-integer for a numeric field, non-boolean for a boolean field) or an unsupported operator SHALL be `400`.

#### Scenario: Unknown field
- **WHEN** a client queries `bogusField==1`
- **THEN** the response is `400`

### Requirement: Tag membership term
On targets and distribution sets, `tag` SHALL test tag membership, accepting only `==`, `!=`, `=in=` and `=out=`. Negation SHALL apply to membership, so `tag!=beta` excludes everything tagged `beta`.

#### Scenario: Negated tag
- **WHEN** a client queries `tag!=beta`
- **THEN** a target tagged both `beta` and `stable` is excluded

### Requirement: Attribute term
On targets, `attribute.<key>` SHALL filter on device-reported attributes. An unknown key SHALL match no targets rather than error.

#### Scenario: Hardware revision
- **WHEN** a client queries `attribute.hw_revision==rev-*`
- **THEN** targets reporting a matching `hw_revision` are returned

### Requirement: One query compiler everywhere
List `q=`, saved target filters, rollout target filters, statistics scoping and polling-interval rules SHALL share one FIQL dialect and one target field map.

#### Scenario: Same query in a filter
- **WHEN** `group==plant-a/*` works on `/rest/v1/targets`
- **THEN** the same string is valid as a target filter query

### Requirement: hawkBit error bodies and status codes
Errors SHALL use hawkBit's body `{"exceptionClass","errorCode","message"}`, with hawkBit's strings for covered cases (including its `entitiy` spelling). Status codes SHALL be `400` invalid input, `401` bad credentials, `404` unknown entity, `409` duplicate or in use, `410` inactive action, `429` quota exceeded.

#### Scenario: Missing target
- **WHEN** a client fetches an unknown target
- **THEN** the response is `404` with `errorCode` `hawkbit.server.error.repo.entitiyNotFound`

### Requirement: Successful deletes return 204
Management API deletes SHALL return `204 No Content`, matching hawkBit 0.10+.

#### Scenario: Delete tag
- **WHEN** an operator deletes a target tag
- **THEN** the response is `204`
