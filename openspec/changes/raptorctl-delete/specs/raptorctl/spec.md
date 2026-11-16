# Spec Delta

## MODIFIED Requirements

### Requirement: Safe destructive commands
Destructive commands (`ds invalidate`, `rollout stop`, `rollout delete`) SHALL ask for confirmation when stdin is a TTY unless `-y` is given. `ds invalidate` SHALL leave actions and rollouts alone unless asked otherwise. `ds delete` and `module delete` SHALL NOT prompt: the server refuses them while an action or a distribution set references the entity.

#### Scenario: Scripted stop
- **WHEN** `rollout stop` runs with stdin not a TTY
- **THEN** it proceeds without prompting

#### Scenario: Delete a rollout on a TTY
- **WHEN** an operator runs `raptorctl rollout delete 3` without `-y`
- **THEN** it names the rollout and asks before deleting it

#### Scenario: Delete a set an action references
- **WHEN** `raptorctl ds delete 7` runs and an action references set 7
- **THEN** the server's `409` is reported and nothing is deleted

### Requirement: Terminal UI
`raptorctl tui` SHALL show the fleet, the selected target's status and actions, and running rollouts, refresh on an interval (default 5 s, `0` off), honour `NO_COLOR`, and mark data stale after a failed refresh. `Tab` SHALL move focus between the targets and rollouts panels, the focused panel carrying the accent border; target keys SHALL act only while targets are focused. `Del` SHALL delete the highlighted distribution set in the assign picker, or the selected rollout while rollouts are focused, each only after a `y` to a prompt naming it.

#### Scenario: Server unreachable
- **WHEN** a refresh fails or times out
- **THEN** the header shows `⚠ stale` until a fetch succeeds

#### Scenario: Delete a rollout from the TUI
- **WHEN** rollouts are focused and the operator presses `Del` on rollout 3
- **THEN** a prompt names rollout 3, `y` deletes it, and `n` or `Esc` sends nothing

#### Scenario: Target keys while rollouts are focused
- **WHEN** rollouts are focused and the operator presses `a`
- **THEN** no assign picker opens
