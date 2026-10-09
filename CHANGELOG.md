# Changelog

All notable changes to raptor are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
raptor adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Within the 1.x series the hawkBit wire format — JSON field names, the paging
envelope, error bodies, and the DDI link structure — is a compatibility
contract; everything on the roadmap is additive.

This file is the scannable index. Each version's
[GitHub release](https://github.com/rosterloh/raptor/releases) carries the full
prose notes, including upgrade guidance and the reasoning behind individual
decisions.

## [1.3.0] - unreleased

### Added

- Dynamic rollouts: `dynamic` on rollout creation appends a trailing group that
  keeps absorbing targets which start matching the filter after creation,
  deploying to them as they arrive. Optional `dynamicGroupTemplate`
  (`{nameSuffix, targetCount}`) sizes and names those groups; a full one rolls
  over to the next. Surfaced as `dynamic` on rollout and group payloads, in the
  console, and in `raptorctl rollout list` (#18)
- `raptorctl ds invalidate <id> [--cancel-rollouts] [--cancel-actions
  none|soft|force] [--yes]`, so withdrawing a release no longer drops out of
  the CLI into hand-written `curl` with credentials on the command line.
  Defaults to the safe path, prints what was withdrawn, and confirms on a TTY
  (#141)
- `raptorctl rollout create|get|start|pause|resume|stop`, so a rollout — and a
  dynamic one's mandatory stop — no longer needs hand-written `curl`. `create`
  flattens the conditions and dynamic-group template into flags; `stop`
  confirms on a TTY unless `-y` (#149)
- The server log at the default level now records Management API writes and
  domain events (registration, actions, device outcomes, rollout progress,
  auth failures); polls and reads stay at `debug` (#155)
- Actions accept hawkBit's `createdAt` and `lastModifiedAt` as `sort=` and
  `q=` fields (#TBD)

### Changed

- A dynamic rollout never reaches `finished` on its own — there may always be
  another device about to match — so ending one is an operator action
  (`POST /rest/v1/rollouts/{id}/stop`). Static rollouts are unaffected (#18)
- `raptorctl ds get` prints `valid`, which is how an invalidated set is
  distinguished from a deployable one (#141)
- Console: sorting the targets, actions and rollouts lists orders the whole
  result server-side, not just the visible page; rollouts can no longer be
  sorted by progress (#TBD)

### Fixed

- Console: jumping from one target's page to another (command palette, Back)
  no longer keeps showing the first target's data (#TBD)
- Console: dialogs closed by navigating away now return keyboard focus (#TBD)

### Fixed

- One target that cannot take a distribution set (e.g. an incompatible target
  type) no longer stops the background sweep for every rollout or auto-assign
  filter behind it, stalls its own rollout in `running` with no group running,
  or fails a device's poll or the `autoAssignDS` request; it is logged,
  counted in a new `raptor.sweep.skipped` metric, and skipped (#148)
- A static rollout group with no actions to measure — every member skipped or
  already on the set — finishes instead of holding the rollout in `running`
  forever, as in hawkBit (#148)
- `raptorctl tui`: an assign, tag, cancel or force acts on the target chosen
  when its prompt opened, not whichever row a refresh left selected; stale or
  hung fetches no longer overwrite a newer filter or spin forever; SIGTERM/SIGHUP
  restore the terminal; the list keeps its scroll position (#158)

## [1.2.0] - 2026-09-07

Closes the last four gaps on the deployment path against hawkBit 1.x, and adds
the two operational features hawkBit 0.10/1.1 introduced after raptor 1.0.

### Added

- Maintenance windows on direct assignments: `maintenanceWindow`
  (`{schedule, duration, timezone}`) on `POST /rest/v1/targets/{cid}/assignedDS`,
  surfaced to devices as DDI `deployment.maintenanceWindow`. Quartz cron
  schedules, strict parsing, availability computed per request (#7)
- Rollout approval workflow: `rollout_approval_enabled` gates new rollouts into
  `waiting_for_approval`; approve/deny with a remark over the API, `raptorctl
  rollout approve|deny`, or the console (#17, #129)
- Rollout stop: `POST /rest/v1/rollouts/{id}/stop`, soft-cancelling issued
  actions and settling to a terminal `stopped` once none is active (#90)
- Target groups: one `group` per target, FIQL-queryable with wildcards
  (`q=group==plant-a/*`), shown and editable in the console and `raptorctl`
  (#89, #117)
- Polling-time overrides: `[ddi] polling_interval` extended to hawkBit 0.10's
  `pollingTime` grammar — ordered `<RSQL> -> <interval>` rules with optional
  per-poll jitter, validated at startup (#91)
- FIQL `autoConfirm==` on targets, matching hawkBit 1.1's RSQL field (#92)
- Per-entity quotas: a `[quota]` section carrying hawkBit's
  `hawkbit.server.security.dos.*` defaults, breaches reported as `429` with
  `hawkbit.server.error.quota.tooManyEntries` (#14)
- Automatic action cleanup: a `[cleanup]` section, off by default, deleting
  closed actions and their status history past a retention window. Deletes
  innermost-first so no orphaned rows survive, and preserves rollout progress
  across a sweep, which upstream does not (#14)
- Multi-tenancy groundwork: a `tenant` column and composite unique keys on every
  query-root table, plus DDI rejection of any tenant but the configured one,
  closing a `controllerId`-collision hazard. raptor remains single-tenant (#12)
- `raptorctl`: tag create/list/delete, target types, and both types named on a
  compatibility mismatch

### Changed

- **Quotas are enforcing on upgrade**, with hawkBit's default values. An
  instance already above one of them — most plausibly a long-lived action past
  1000 status entries — will see `429` on writes that would push it further. Set
  the relevant `[quota]` key to `0` to restore 1.1.0's unbounded behaviour;
  nothing is deleted retroactively (#14)
- Web console usability and responsive navigation pass: confirmation on
  consequential controls, labelled and validated release forms, persisted table
  sorting, grouped responsive navigation, standardised list pages, clearer
  rollout progress semantics, unambiguous timestamps, better pagination (#127)
- `raptorctl` TUI: event-driven loop in place of polling redraw, OSC 52 yank,
  table layout with clipped columns, status chips, rounded panels
- argon2 0.5 → 0.6, croner 3 → 4, SeaORM/tower-http/toml minor bumps
- CI: sqlite and postgres jobs cut from ~237s to ~48s of work

### Fixed

- `raptorctl publish` conflated the software-module and distribution-set type
  flags; they are now separate
- croner 4 rejects Quartz's `0/15` stepped-range spelling by default, which
  would have turned working operator-supplied `maintenanceWindow.schedule`
  values into `400`s. `sloppy_ranges` keeps them working, pinned by tests

## [1.1.0] - 2026-08-09

Finishes the web console redesign begun in 1.0.0 and closes most remaining
console gaps against hawkBit 1.x. Ships `raptorctl` as a new client.

### Added

- `raptorctl`: a CLI and terminal UI for the Management API, including
  `publish` for streaming artifact uploads with SHA-256 verification (#74)
- Console: type management for software-module, distribution-set and target
  types, including DS-type composition and target-type compatibility (#34)
- Console: confirmation flow — auto-confirm toggle and a distinct "waiting for
  confirmation" badge (#25)
- Console: metadata CRUD for targets, modules and distribution sets, with the
  module-only `targetVisible` toggle (#35)
- Console: distribution-set edit and invalidate, with cancel-rollouts and
  cancel-actions options; invalid sets are flagged and cannot be redeployed (#36)
- Console: light theme honouring `prefers-color-scheme` with a persisted
  override (#68), and a ⌘K command palette (#69)
- Console: URL-addressable list state, so Back, bookmarking and refresh preserve
  filters and pagination (#81)
- Console: reinstall-loop diagnostic — actions re-fetching `deploymentBase`
  without ever reporting status get a "fetched N×, no feedback" badge (#77)
- Target attributes are FIQL-queryable on `/rest/v1/targets` (#66)
- `/rest/v1/system/statistics` accepts `q=` to scope fleet counters to a saved
  filter (#63)
- Compatibility matrix pinned to hawkBit 1.x, plus the swupdate/suricatta
  integration guide (#78, #96)

### Changed

- Console dashboard, targets list and target detail rebuilt on a new
  design-token foundation (#64, #65)
- Target and distribution-set list responses embed the assigned set and tags,
  cutting the console's per-row fan-out (#70)
- The tags page reads assignment counts in one request instead of one `count`
  call per row (#67)

### Fixed

- Deleting a software module still referenced by a distribution set now refuses
  with a conflict instead of silently breaking the set (#75)
- Accessibility and polish: real focus trap and focus restore on dialogs (#86),
  notification auto-dismiss with screen-reader announcements, keyboard-reachable
  table rows, real modal semantics, debounced search (#84), per-page tab titles
  (#83), a catch-all 404 route (#82)

## [1.0.0] - 2026-07-29

The hawkBit compatibility surface becomes a contract rather than a moving
target. Stock DDI clients — SWUpdate, RAUC `hawkbit-updater`, Zephyr's hawkBit
module, `hawkbit-rs` — work against raptor unchanged, and the wire format they
depend on is frozen for the 1.x series. No breaking changes from 0.9.0 and no
manual migration steps.

### Added

- All four hawkBit action types — `forced`, `soft`, `downloadonly`,
  `timeforced` — with operator escalation
  (`PUT /rest/v1/targets/{cid}/actions/{id}`) and force-quit
  (`DELETE ...?force=true`). Rollouts carry an action type too (#4)
- Server-side dashboard statistics via `GET /rest/v1/system/statistics`,
  replacing an in-browser tally that went wrong past 500 targets, plus a
  read-only system configuration card (#38)
- `auto_confirm_default`, auto-confirming newly registered targets when the
  confirmation flow is enabled (#39–#44)
- Zephyr integration guide with a per-feature compatibility matrix

### Fixed

DDI compatibility pass, found by running Zephyr's hawkBit client against raptor
(#39–#44):

- The confirmation flow no longer strands clients that do not implement
  `confirmationBase`, which would otherwise poll forever without installing
- `configData` is advertised only until the device has reported its attributes,
  instead of on every poll
- Artifact `download-http` links carry plain HTTP rather than inheriting the
  request's scheme
- The device address (`ipUri`) is recorded from controller polls
- The `DEFAULT`-tenant constraint in emitted links is documented and guarded

### Changed

- SeaORM 2.0, base64 0.23, and the usual minor/patch sweep

---

Releases before 1.0.0 predate this file. See the
[git history](https://github.com/rosterloh/raptor/commits/main) and the
[0.9.0 tag](https://github.com/rosterloh/raptor/releases/tag/v0.9.0).

[1.2.0]: https://github.com/rosterloh/raptor/compare/v1.1.0...v1.2.0
[1.1.0]: https://github.com/rosterloh/raptor/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/rosterloh/raptor/compare/v0.9.0...v1.0.0
