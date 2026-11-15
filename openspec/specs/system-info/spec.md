# system-info Specification

## Purpose

Read-only system endpoints: the hawkBit tenant configuration derived from the
config file, and fleet statistics.

## Requirements

### Requirement: Tenant configuration is read-only
`GET /rest/v1/system/configs` SHALL report hawkBit keys derived from the config file, including `pollingTime`, `authentication.gatewaytoken.enabled`, `authentication.targettoken.enabled`, `user.confirmation.flow.enabled`, `rollout.approval.enabled`, the cleanup keys and `multi.assignments.enabled` (false). Each SHALL be marked global. Writes to a key SHALL be `403`.

#### Scenario: Write attempt
- **WHEN** a client PUTs `/rest/v1/system/configs/pollingTime`
- **THEN** the response is `403`

### Requirement: Fleet statistics
`GET /rest/v1/system/statistics` SHALL return `totalTargets`, `targetsByStatus`, `totalDistributionSets`, `totalSoftwareModules`, `totalActions`, `activeActions` and `totalRollouts`.

#### Scenario: Dashboard tiles
- **WHEN** the console loads its dashboard
- **THEN** counts come from one statistics request, correct for any fleet size

### Requirement: Statistics scoped by filter
An optional `q=` SHALL narrow only `totalTargets` and `targetsByStatus` to matching targets. Catalogue and action counters SHALL stay fleet-wide. An invalid query SHALL be `400`.

#### Scenario: Tag scope
- **WHEN** a client requests statistics with `q=tag==linux-gw`
- **THEN** target counters cover only `linux-gw` targets and `totalActions` is unchanged
