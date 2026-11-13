# quotas Specification

## Purpose

Per-entity growth caps that mirror hawkBit's DoS quotas, bounding how much any
single entity can accumulate rather than rate-limiting requests.

## Requirements

### Requirement: Configurable caps with hawkBit defaults
`[quota]` SHALL cap status entries per action (1000), messages per status (50), attributes per target (100), metadata per target, module and set (100 each), artifacts per module (50), modules per set (100), groups per rollout (500) and targets per group (20000). A value of `0` SHALL disable that cap.

#### Scenario: Disabled cap
- **WHEN** `max_status_entries_per_action = 0`
- **THEN** a device can report any number of status entries

### Requirement: Breach response
A request that would exceed a cap SHALL be `429` with error code `hawkbit.server.error.quota.tooManyEntries`, changing nothing.

#### Scenario: Too many attributes
- **WHEN** a device reports its 101st attribute with the default cap
- **THEN** the `configData` PUT is `429`

### Requirement: Device caps never block closure
Device-reported caps SHALL apply only to what devices send. Server-written status entries SHALL never be capped, and feedback that closes an action (`closed`, `canceled`, `downloadonly`'s `downloaded`, a cancel's `closed`/`rejected`) SHALL be exempt from the status-entry cap.

#### Scenario: Exhausted budget
- **WHEN** a device at its status-entry cap reports `closed`/`success`
- **THEN** the action finishes normally
