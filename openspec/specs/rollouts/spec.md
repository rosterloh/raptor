# rollouts Specification

## Purpose

Staged deployment of a distribution set across the targets matching a FIQL
filter, group by group, gated by success and error thresholds, with optional
approval and dynamic membership.

## Requirements

### Requirement: Creation
`POST /rest/v1/rollouts` SHALL take a name, set, `targetFilterQuery`, `amountGroups`, success and error `THRESHOLD` percentages, and an action `type` (plus `forcetime`). Matching targets SHALL be split evenly into groups at creation. An invalid query or type SHALL be `400`. An omitted error condition SHALL never trip.

#### Scenario: Three groups
- **WHEN** 9 targets match and `amountGroups` is 3
- **THEN** three groups of 3 targets are created and the rollout is `ready`

### Requirement: Lifecycle operations
`start` SHALL move `ready` → `running` and schedule the first group. `pause` SHALL move `running` → `paused`. `resume` SHALL move `paused` → `running` and re-evaluate. `DELETE` SHALL cancel the rollout's active actions and remove it.

#### Scenario: Start
- **WHEN** an operator starts a ready rollout
- **THEN** only the first group's targets get actions

### Requirement: Threshold evaluation
A background evaluator, every `rollout_eval_interval_secs` (default 5), SHALL finish a running group and schedule the next once its success threshold is met, and pause the group and rollout once its error threshold is reached. The rollout SHALL finish when its last group finishes.

#### Scenario: Error threshold
- **WHEN** group 1's error percentage reaches its threshold
- **THEN** the group and rollout are `paused`

### Requirement: Skipped members
A group member that cannot take the set when its group is scheduled SHALL be skipped with a warning, and thresholds measured against the targets actually deployed to. A group with nothing to measure SHALL finish on the next evaluation.

#### Scenario: Incompatible member
- **WHEN** one member's target type no longer accepts the set
- **THEN** the rest of the group is deployed and that member is skipped

### Requirement: Stop
`stop` SHALL be accepted from `ready`, `waiting_for_approval`, `approval_denied`, `running` and `paused`. It SHALL soft-cancel the rollout's active actions, mark unfinished groups `stopped`, report `stopping` until those cancels resolve, then settle to `stopped`. It SHALL be `400` from `stopping`, `stopped` and `finished`.

#### Scenario: Nothing in flight
- **WHEN** a rollout whose devices have all finished is stopped
- **THEN** it goes straight to `stopped`

### Requirement: Approval gate
With `rollout_approval_enabled`, new rollouts SHALL start in `waiting_for_approval`, where `start` is refused. `approve` SHALL move it to `ready` and `deny` to terminal `approval_denied`. Both take an optional `remark`, return `204`, and record `approveDecidedBy` and `approvalRemark`.

#### Scenario: Deny is final
- **WHEN** an operator denies a rollout
- **THEN** it can never be started, only stopped or deleted

### Requirement: Progress counters
Rollouts and groups SHALL carry `totalTargetsPerStatus` (`notstarted`, `scheduled`, `running`, `error`, `finished`, `cancelled`). A rollout's counts SHALL be the sum of its groups'.

#### Scenario: Waiting group
- **WHEN** group 2 has not been scheduled yet
- **THEN** its targets count as `scheduled`

### Requirement: Dynamic rollouts
`dynamic: true` SHALL add a trailing group that absorbs newly matching targets, sized by `dynamicGroupTemplate.targetCount` (default: the last static group's size) and rolling over into a new group when full, up to the group quota. A dynamic rollout SHALL never finish on its own. A template without `dynamic` SHALL be `400`.

#### Scenario: Late device
- **WHEN** a matching device registers while a dynamic rollout's trailing group is running
- **THEN** it joins that group and is deployed to immediately

### Requirement: Group inspection
Operators SHALL list a rollout's groups, read one group, and list the controller ids in a group.

#### Scenario: Group targets
- **WHEN** an operator GETs `.../deploygroups/5/targets`
- **THEN** the controller ids in group 5 are returned
