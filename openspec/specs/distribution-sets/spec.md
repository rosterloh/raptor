# distribution-sets Specification

## Purpose

Distribution sets are the releasable unit operators assign: named, versioned
bundles of software modules, with completeness, invalidation and metadata rules.

## Requirements

### Requirement: Create with modules
`POST /rest/v1/distributionsets` SHALL take an array of sets with name, version, type key and optional inline modules. Modules MAY be added later via `assignedSM`, up to `max_software_modules_per_distribution_set`.

#### Scenario: Add a module later
- **WHEN** an operator POSTs `[{"id":2}]` to `.../assignedSM`
- **THEN** module 2 joins the set

### Requirement: Completeness
A set SHALL be `complete` when it contains every mandatory module type of its set type. Only complete sets SHALL be assignable, to targets or as auto-assign sets; an incomplete one SHALL be `400`.

#### Scenario: Missing OS module
- **WHEN** an operator assigns an `os` set that has no `os` module
- **THEN** the response is `400`

### Requirement: Deletion guard
A set referenced by any action, even a finished one, SHALL not be deletable.

#### Scenario: Deployed set
- **WHEN** an operator deletes a set that was once deployed
- **THEN** the delete is refused

### Requirement: Invalidation
`POST .../invalidate` SHALL mark the set `valid: false`, block new assignments and rollouts of it, and detach auto-assign filters using it. `actionCancelationType` (`none` default, `soft`, `force`) SHALL control in-flight actions, and `cancelRollouts` SHALL stop its rollouts. Invalidation SHALL not be undoable.

#### Scenario: Second invalidation
- **WHEN** an operator invalidates an already invalid set
- **THEN** the response is `409`

#### Scenario: Bad cancelation type
- **WHEN** `actionCancelationType` is `bogus`
- **THEN** the response is `400`

### Requirement: Set metadata
Sets SHALL support key/value metadata (create as array, list, get, update, delete), capped by `max_metadata_entries_per_distribution_set`.

#### Scenario: Read entry
- **WHEN** an operator GETs `.../metadata/channel`
- **THEN** that entry is returned

### Requirement: Set queries
Set lists SHALL support FIQL over `id`, `name`, `version`, `description`, `complete` and `tag`.

#### Scenario: Complete sets
- **WHEN** an operator queries `complete==true`
- **THEN** only complete sets are returned
