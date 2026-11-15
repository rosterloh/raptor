# data-retention Specification

## Purpose

Opt-in background sweeps that bound long-term growth: deleting old closed
actions, and retiring old releases together with their artifacts.

## Requirements

### Requirement: Action cleanup is opt-in
`[cleanup]` SHALL be off by default. When `enabled`, every `interval_secs` (3600) it SHALL delete inactive actions whose status is in `action_statuses` (default `finished`, `downloaded`, `error`, `canceled`) and whose last change is older than `action_expiry_days` (30), with their status history.

#### Scenario: Old finished action
- **WHEN** cleanup is enabled and an action finished 40 days ago
- **THEN** the next sweep deletes it and its history

### Requirement: Active actions are never cleaned
An active action SHALL never be deleted, whatever its status.

#### Scenario: Active canceling action
- **WHEN** an active action is older than the expiry window
- **THEN** it is kept

### Requirement: Bounded cleanup batches
Cleanup SHALL delete at most 1000 actions per statement and 10000 per sweep.

#### Scenario: Large backlog
- **WHEN** 50000 actions are eligible
- **THEN** they drain over several sweeps

### Requirement: Rollout progress survives cleanup
Deleting a rollout's actions SHALL not change its `totalTargetsPerStatus`: purged outcomes SHALL be recorded per group first.

#### Scenario: Finished rollout
- **WHEN** cleanup deletes a finished rollout's actions
- **THEN** its counters still show those targets as finished

### Requirement: Cleanup is reported as hawkBit config
`action.cleanup.auto.expiry` (ms, `-1` when disabled) and `action.cleanup.auto.status` SHALL be reported on `/rest/v1/system/configs`.

#### Scenario: Disabled
- **WHEN** cleanup is off
- **THEN** `action.cleanup.auto.expiry` is `-1`

### Requirement: Release retention
Setting `[retention] keep_releases` (at least 1) SHALL keep the newest N valid sets per set name by creation time. Older sets SHALL be retired every `interval_secs`, at most 50 per sweep. Unset SHALL keep everything. Invalidated sets SHALL not use a kept slot.

#### Scenario: Keep three
- **WHEN** `keep_releases = 3` and a name has 5 valid sets
- **THEN** the two oldest are retired

### Requirement: Retiring a set
Retirement SHALL invalidate the set with force and cancel its rollouts, then delete its rollouts, actions, metadata, tag links and the set. Targets that had it installed SHALL show nothing installed and return to `registered`. Modules no remaining set uses SHALL be deleted, and their blobs only when unreferenced.

#### Scenario: Shared module
- **WHEN** a retired set shares a module with a kept set
- **THEN** that module and its artifacts are kept
