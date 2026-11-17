# Spec Delta

## MODIFIED Requirements

### Requirement: Assignment creates an action
`POST /rest/v1/targets/{cid}/assignedDS` with `{"id", "type"?, "externalRef"?}` SHALL create an action (`running`, or `wait_for_confirmation` under the confirmation flow), set the target `pending` and its assigned set, and store `externalRef` on the action. The set SHALL be complete, valid and type-compatible, and `externalRef` at most 512 characters, else `400`.

#### Scenario: Forced assignment
- **WHEN** an operator assigns set 1 with `type: forced`
- **THEN** a `running` forced action exists and the target is `pending`

#### Scenario: External reference
- **WHEN** an operator assigns set 1 with `externalRef: job-42`
- **THEN** the new action reports `externalRef: job-42`

### Requirement: Fleet-wide action list
`GET /rest/v1/actions` SHALL list every action with paging, sorting and FIQL over `id`, `active`, `detailStatus`, `createdAt`, `lastModifiedAt`, `lastStatusCode` and `externalRef`.

#### Scenario: Failed deployments
- **WHEN** an operator queries `detailStatus==error`
- **THEN** only failed actions are returned

#### Scenario: By last status code
- **WHEN** an operator queries `lastStatusCode==42`
- **THEN** only actions whose last device report carried code 42 are returned
