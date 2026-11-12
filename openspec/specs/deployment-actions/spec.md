# deployment-actions Specification

## Purpose

Operator-side assignment of distribution sets to targets, and the action
records that track each deployment: action types, maintenance windows, the
one-active-action invariant, inspection, escalation and cancellation.

## Requirements

### Requirement: Assignment creates an action
`POST /rest/v1/targets/{cid}/assignedDS` with `{"id", "type"?}` SHALL create an action (`running`, or `wait_for_confirmation` under the confirmation flow), set the target `pending` and its assigned set. The set SHALL be complete, valid and type-compatible, else `400`.

#### Scenario: Forced assignment
- **WHEN** an operator assigns set 1 with `type: forced`
- **THEN** a `running` forced action exists and the target is `pending`

### Requirement: Action types
`type` SHALL be `forced` (default), `soft`, `timeforced` (with `forcetime` in epoch millis; omitted means already reached) or `downloadonly`. Any other type SHALL be `400`. The request field is `forcetime` and the response field is `forceTime`, as in hawkBit.

#### Scenario: Unknown type
- **WHEN** an operator assigns with `type: later`
- **THEN** the response is `400`

### Requirement: One active action per target
A target SHALL have at most one active action. A new assignment SHALL cancel the previous active action and record why.

#### Scenario: Reassign mid-deployment
- **WHEN** a set is assigned to a target whose previous action is still running
- **THEN** the old action is cancelled and the new one is active

### Requirement: Maintenance windows
An assignment MAY carry `maintenanceWindow` with a Quartz cron `schedule`, a `duration` (`HH:mm:ss`, at most `23:59:59`) and a fixed `timezone` offset (`±HH:mm`). A window that cannot be evaluated SHALL be `400`. The action SHALL echo the window with `nextStartAt`.

#### Scenario: Unix cron rejected
- **WHEN** `schedule` is the five-field `0 2 * * 1`
- **THEN** the response is `400`

### Requirement: Action states
An action SHALL be in one of `wait_for_confirmation`, `running`, `canceling` (active), or `canceled`, `finished`, `error` (inactive). API `status` SHALL be `pending` while active and `finished` otherwise, with the fine state in `detailStatus`. Every transition and feedback SHALL append a status entry.

#### Scenario: Status history
- **WHEN** an operator lists `.../actions/{aid}/status`
- **THEN** entries come oldest first, with `type`, `messages` and `reportedAt`, and accept paging and `sort`

### Requirement: Escalation
`PUT .../actions/{aid}` with `{"forceType":"forced"}` SHALL make an active action forced from the next fetch. Escalating an inactive action SHALL be `410`.

#### Scenario: Escalate soft action
- **WHEN** an operator escalates a running soft action
- **THEN** the device's next `deploymentBase` says `forced`

### Requirement: Cancellation
`DELETE .../actions/{aid}` SHALL move an active action to `canceling` for the device to acknowledge. With `?force=true` it SHALL close the action as `canceled` immediately.

#### Scenario: Force cancel
- **WHEN** an operator force-cancels an action of an offline device
- **THEN** the action is `canceled` without device involvement

### Requirement: Fleet-wide action list
`GET /rest/v1/actions` SHALL list every action with paging, sorting and FIQL over `id`, `active`, `detailStatus`, `createdAt` and `lastModifiedAt`.

#### Scenario: Failed deployments
- **WHEN** an operator queries `detailStatus==error`
- **THEN** only failed actions are returned

### Requirement: Fetch-loop indicator
Every action SHALL expose `deploymentFetchCount` (raptor extension): `deploymentBase` fetches since the device's last feedback of any kind.

#### Scenario: Reinstall loop
- **WHEN** a device fetches `deploymentBase` five times without reporting
- **THEN** `deploymentFetchCount` is 5
