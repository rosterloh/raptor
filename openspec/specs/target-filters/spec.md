# target-filters Specification

## Purpose

Saved, named FIQL target queries that can carry an auto-assign distribution set,
turning them into rules that deploy to matching targets now and as they appear.

## Requirements

### Requirement: Filter CRUD
Target filters SHALL have a unique name and a query validated against the target field map when written. An invalid query SHALL be `400` and a duplicate name `409`.

#### Scenario: Invalid query
- **WHEN** an operator saves a filter with query `nope==1`
- **THEN** the response is `400`

### Requirement: Attaching an auto-assign set
`POST .../autoAssignDS` with `{"id","type"}` (`forced` default, or `soft`) SHALL attach a complete, valid set and immediately assign it to currently matching targets. An incomplete set SHALL be `400`. `GET` SHALL return the attached set or `204`, and `DELETE` SHALL detach it.

#### Scenario: Attach
- **WHEN** an operator attaches set 1 to a filter matching 10 idle targets
- **THEN** those 10 targets get actions for set 1

### Requirement: When auto-assignment runs
Auto-assignment SHALL run on DDI registration (in the same poll), on attribute change, and in a periodic sweep sharing the rollout evaluator's interval.

#### Scenario: Created via API
- **WHEN** a matching target is created through the Management API
- **THEN** the next sweep assigns it

### Requirement: Non-disruptive assignment
Auto-assignment SHALL skip a target that already has the set assigned, has any active action, or has ever had an action for that set. An operator's cancel SHALL therefore stick.

#### Scenario: Cancelled auto-assignment
- **WHEN** an operator cancels an auto-assigned action
- **THEN** later sweeps do not assign that set to that target again

### Requirement: Incompatible targets are skipped
A matching target that cannot take the set SHALL be skipped with a warning on every sweep, without failing the poll, the sweep or the attach request.

#### Scenario: Wrong target type
- **WHEN** a matching target's type does not accept the set's type
- **THEN** it is skipped and other targets are still assigned
