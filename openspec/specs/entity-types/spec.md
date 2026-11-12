# entity-types Specification

## Purpose

Software-module types, distribution-set types and target types: what kinds of
module a set must contain, and which kinds of set a device may accept.

## Requirements

### Requirement: Seeded types
raptor SHALL seed module types `os`, `firmware`, `runtime`, `application` and set types `os`, `os_app`, `app`, each set type with its mandatory module types.

#### Scenario: Fresh database
- **WHEN** an operator lists module types on a new install
- **THEN** the four seeded types are present

### Requirement: Type CRUD
Module types, set types and target types SHALL support create, list, get, update and delete. Module and set type updates SHALL change only the description. Deleting a type still in use SHALL be `409`.

#### Scenario: Delete in-use type
- **WHEN** an operator deletes the `os` module type while modules use it
- **THEN** the response is `409`

### Requirement: Set type composition
A set type SHALL have mandatory and optional module types, editable via `.../mandatorymoduletypes` and `.../optionalmoduletypes`. Mandatory types SHALL drive set completeness.

#### Scenario: New mandatory type
- **WHEN** an operator adds `firmware` as mandatory to a set type
- **THEN** that type's sets without a firmware module become incomplete

### Requirement: Target type compatibility
A target type SHALL list compatible set types, editable via `.../compatibledistributionsettypes`. Assigning a set to a typed target SHALL require its set type to be compatible.

#### Scenario: Compatible
- **WHEN** a target's type accepts `os` and an `os` set is assigned
- **THEN** the assignment succeeds
