# ddi-deployment Specification

## Purpose

What a device receives when told to deploy, and how its feedback drives the
action state machine: `deploymentBase`, `installedBase`, deployment feedback and
the cancel handshake, field-for-field with hawkBit DDI v1.

## Requirements

### Requirement: deploymentBase payload
`GET .../deploymentBase/{actionId}` SHALL return hawkBit's deployment JSON: `id`, `deployment.download`, `deployment.update`, `deployment.chunks` (one per module, with `part`, `version`, `name`, `artifacts` and target-visible `metadata`), and `actionHistory`. Each fetch SHALL increment the action's `deploymentFetchCount`.

#### Scenario: Chunk metadata
- **WHEN** a module has no metadata marked `targetVisible`
- **THEN** its chunk omits the `metadata` key

### Requirement: Chunk part names
A chunk's `part` SHALL come from the module type key: `application` → `bApp`, `os` and `firmware` → `os`, `runtime` → `jvm`, and any other key unchanged.

#### Scenario: Application module
- **WHEN** a set contains a module of type `application`
- **THEN** its chunk has `"part": "bApp"`

### Requirement: Handling modes follow the action type
`download`/`update` SHALL be `forced`/`forced` for `forced`, `attempt`/`attempt` for `soft` and for `timeforced` before `forcetime`, `forced`/`forced` for `timeforced` after it, and `forced`/`skip` for `downloadonly`. They SHALL be computed per request.

#### Scenario: Timeforced deadline passes
- **WHEN** a `timeforced` action's `forcetime` passes between two fetches
- **THEN** the first fetch says `attempt` and the second says `forced`

### Requirement: Maintenance window field
For an action with a maintenance window, `deployment.maintenanceWindow` SHALL be `available` while the window is open and `unavailable` while it is shut. While shut, `update` SHALL be `skip`. Actions without a window SHALL omit the key.

#### Scenario: Window shut
- **WHEN** a forced action's window is shut
- **THEN** the device gets `download: forced`, `update: skip`, `maintenanceWindow: unavailable`

### Requirement: Deployment feedback
`POST .../deploymentBase/{actionId}/feedback` SHALL accept `status.execution` (`proceeding`, `scheduled`, `resumed`, `downloading`, `downloaded`, `canceled`, `rejected`, `closed`) and `status.result.finished` (`none`, `success`, `failure`). Each report SHALL append a status history entry and reset `deploymentFetchCount`.

#### Scenario: Progress report
- **WHEN** a device reports `proceeding`
- **THEN** a history entry is added and the action stays active

### Requirement: Terminal feedback
`closed`/`success` SHALL finish the action, set the target `in_sync` and record the set as installed. `closed`/`failure` SHALL set the action and target to `error`. For `downloadonly`, `downloaded` SHALL finish it with `detailStatus: downloaded`, leaving the installed set unchanged.

#### Scenario: Successful install
- **WHEN** a device reports `closed` with `success`
- **THEN** the action is `finished`, the target is `in_sync`, and `installedBase` shows the set

### Requirement: Feedback for inactive actions
Feedback for an action that is no longer active SHALL be rejected with `410 Gone` and the hawkBit `actionNotActive` error code.

#### Scenario: Late report
- **WHEN** a device posts feedback for an already finished action
- **THEN** the response is `410`

### Requirement: Progress reporting
`status.result.progress` (`{"cnt","of"}`) SHALL be accepted and published to live events, never persisted. A malformed value SHALL be ignored without failing the feedback.

#### Scenario: Malformed progress
- **WHEN** feedback carries `"progress": "half"`
- **THEN** the feedback is accepted and the progress is dropped

### Requirement: Cancel handshake
`GET .../cancelAction/{actionId}` SHALL return `{"id","cancelAction":{"stopId"}}` while the action is `canceling`, else `404`. Cancel feedback `closed` SHALL make it `canceled` and revert the assigned set to the installed one. `rejected` SHALL put it back to `running`. Other values SHALL only be recorded.

#### Scenario: Device confirms cancel
- **WHEN** a device posts `closed` to `cancelAction/7/feedback`
- **THEN** action 7 is `canceled`

#### Scenario: Device refuses cancel
- **WHEN** a device posts `rejected` to `cancelAction/7/feedback`
- **THEN** action 7 is `running` again

### Requirement: installedBase
`GET .../installedBase/{actionId}` SHALL replay the last successfully installed deployment in `deploymentBase` shape, without a `maintenanceWindow` field.

#### Scenario: After install
- **WHEN** a device fetches `installedBase` for its finished action
- **THEN** it gets that deployment's chunks
