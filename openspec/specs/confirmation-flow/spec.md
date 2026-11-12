# confirmation-flow Specification

## Purpose

hawkBit's optional confirmation flow: new assignments wait for the device, or a
per-target auto-confirm, to confirm before they deploy.

## Requirements

### Requirement: Server-wide toggle
With `[ddi] confirmation_flow` true, a new assignment to a target without auto-confirm SHALL start in `wait_for_confirmation`. When false (default) it SHALL start `running`. raptor SHALL log a startup warning when the flow is on.

#### Scenario: Flow on
- **WHEN** the flow is on and an operator assigns a set to a target without auto-confirm
- **THEN** the action is `wait_for_confirmation` and the poll offers `confirmationBase`, not `deploymentBase`

### Requirement: confirmationBase payload
`GET .../confirmationBase/{actionId}` SHALL return the same payload as `deploymentBase`, keyed `confirmation` instead of `deployment`.

#### Scenario: Fetch
- **WHEN** a device fetches `confirmationBase/7`
- **THEN** the body has `confirmation.chunks` describing action 7

### Requirement: Device confirm and deny
`POST .../confirmationBase/{actionId}/feedback` with `confirmed` SHALL move the action to `running`. `denied` SHALL record the denial and leave a waiting action waiting. Any other value SHALL be `400`.

#### Scenario: Deny
- **WHEN** a device denies a waiting action
- **THEN** the action stays `wait_for_confirmation` with a `denied` history entry

### Requirement: Device revokes a confirmation
`denied` on a `running` action SHALL move it back to `wait_for_confirmation` and record `confirmation revoked by device`, as in hawkBit 1.2.

#### Scenario: Revoke
- **WHEN** a device denies its running action
- **THEN** its next poll offers `confirmationBase` again

### Requirement: Auto-confirm
A target SHALL have an `autoConfirm` flag, toggled by operators (`/rest/v1/targets/{cid}/autoConfirm/activate|deactivate`) or by the device (`confirmationBase/activateAutoConfirm|deactivateAutoConfirm`). Activating it SHALL release the target's waiting actions to `running`.

#### Scenario: Release pending
- **WHEN** an operator activates auto-confirm on a target with a waiting action
- **THEN** that action moves to `running`

### Requirement: Auto-confirm default for new targets
`[ddi] auto_confirm_default` SHALL set `autoConfirm` on targets created afterwards, by DDI registration or the Management API. Existing targets SHALL keep their flag.

#### Scenario: Mixed fleet
- **WHEN** `auto_confirm_default = true` and a new device registers
- **THEN** its assignments skip `wait_for_confirmation`
