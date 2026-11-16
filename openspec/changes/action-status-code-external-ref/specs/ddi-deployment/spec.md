# Spec Delta

## MODIFIED Requirements

### Requirement: Deployment feedback
`POST .../deploymentBase/{actionId}/feedback` SHALL accept `status.execution` (`proceeding`, `scheduled`, `resumed`, `downloading`, `downloaded`, `canceled`, `rejected`, `closed`), `status.result.finished` (`none`, `success`, `failure`) and an optional integer `status.code`. Each report SHALL append a status history entry carrying its code, reset `deploymentFetchCount`, and set the action's `lastStatusCode` to the report's code, or clear it when the report has none.

#### Scenario: Progress report
- **WHEN** a device reports `proceeding`
- **THEN** a history entry is added and the action stays active

#### Scenario: Report with a status code
- **WHEN** a device reports `closed`/`failure` with `code: 42`
- **THEN** the history entry has `code: 42` and the action's `lastStatusCode` is `42`

#### Scenario: Report without a status code
- **WHEN** a device that last reported `code: 42` reports `proceeding` with no code
- **THEN** the action has no `lastStatusCode`

### Requirement: Cancel handshake
`GET .../cancelAction/{actionId}` SHALL return `{"id","cancelAction":{"stopId"}}` while the action is `canceling`, else `404`. Cancel feedback `closed` SHALL make it `canceled` and revert the assigned set to the installed one. `rejected` SHALL put it back to `running`. Other values SHALL only be recorded. Cancel feedback SHALL record `status.code` as deployment feedback does.

#### Scenario: Device confirms cancel
- **WHEN** a device posts `closed` to `cancelAction/7/feedback`
- **THEN** action 7 is `canceled`

#### Scenario: Device refuses cancel
- **WHEN** a device posts `rejected` to `cancelAction/7/feedback`
- **THEN** action 7 is `running` again
