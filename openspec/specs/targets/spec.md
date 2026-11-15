# targets Specification

## Purpose

Operator management of targets (devices): creation, update and deletion, update
status, groups, attributes, metadata, target types, and the embedded summary
fields list views rely on.

## Requirements

### Requirement: Create targets in bulk
`POST /rest/v1/targets` SHALL take a JSON array and create each target, generating a `securityToken` when none is given. New targets SHALL have status `unknown` and `requestAttributes` set. A duplicate `controllerId` SHALL be `409`.

#### Scenario: Generated token
- **WHEN** an operator creates `[{"controllerId":"dev-42"}]`
- **THEN** `dev-42` exists with a non-empty `securityToken`

### Requirement: Update status values
A target's `updateStatus` SHALL be one of `unknown` (never polled), `registered`, `pending` (update in progress), `in_sync`, or `error`.

#### Scenario: Assignment
- **WHEN** a set is assigned to a `registered` target
- **THEN** its status becomes `pending`

### Requirement: Partial update
`PUT /rest/v1/targets/{cid}` SHALL change only the fields present: name, description, securityToken, `requestAttributes`, `group`. An omitted field SHALL be left unchanged; there is no way to clear `group` or `description`.

#### Scenario: Move group
- **WHEN** an operator PUTs `{"group":"plant-b/line-1"}`
- **THEN** only the group changes

### Requirement: Delete cascades
`DELETE /rest/v1/targets/{cid}` SHALL delete the target with its actions, attributes, metadata, tag links and rollout membership.

#### Scenario: Delete
- **WHEN** an operator deletes a target that has actions
- **THEN** the target and those actions are gone

### Requirement: Groups
A target SHALL carry at most one free-form `group` string. `/` is an ordinary character and nesting is a naming convention, queried with wildcards.

#### Scenario: Site query
- **WHEN** an operator queries `group==plant-a/*`
- **THEN** targets in every line of plant A match

### Requirement: Attributes are device-owned
`GET /rest/v1/targets/{cid}/attributes` SHALL return the device-reported attributes as a flat object. Operators SHALL not write them.

#### Scenario: Read attributes
- **WHEN** a device has reported `hw=rev2`
- **THEN** the attributes endpoint returns `{"hw":"rev2"}`

### Requirement: Target metadata
Targets SHALL support operator key/value metadata: create (array), list, get, update and delete per key, capped by `max_metadata_entries_per_target`.

#### Scenario: Duplicate key
- **WHEN** an operator creates a metadata key that already exists on the target
- **THEN** the response is `409`

### Requirement: Target type assignment
A target SHALL have at most one target type, set and cleared via `/rest/v1/targets/{cid}/targettype`. An assignment of a set whose type is not compatible with the target type SHALL be `400`.

#### Scenario: Incompatible set
- **WHEN** a `gateway` target accepting only `os` sets is assigned an `app` set
- **THEN** the response is `400` naming the accepted types

### Requirement: Embedded summary fields
Target list and get responses SHALL embed `installedDs` and `assignedDs` (`id`, `name`, `version`, type key) and `tags`, omitting each when absent. These are a raptor extension, resolved in a fixed number of queries per page.

#### Scenario: Nothing installed
- **WHEN** a target has never installed anything
- **THEN** its JSON has no `installedDs` key

### Requirement: Poll status
A target that has polled SHALL include `pollStatus` with the last request time, the next expected time from the default polling interval, and an `overdue` flag.

#### Scenario: Quiet device
- **WHEN** a target's next expected poll time has passed
- **THEN** `pollStatus.overdue` is true

### Requirement: Assigned and installed sets
`GET .../assignedDS` and `GET .../installedDS` SHALL return the relevant set, or `204` when there is none.

#### Scenario: Never installed
- **WHEN** an operator reads `installedDS` for a new target
- **THEN** the response is `204`
