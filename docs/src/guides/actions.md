# Assignments & Actions

An **action** is the record of one deployment: a distribution set being rolled
out to one target. Assigning a DS creates an action; the device's feedback drives
it to completion.

## Assigning a distribution set

```bash
curl -u admin:pw -X POST localhost:8088/rest/v1/targets/device-42/assignedDS \
  -H 'Content-Type: application/json' -d '{"id":1,"type":"forced"}'
```

The `type` is the action type. All four of hawkBit's are supported, and each maps
to the `download`/`update` handling modes the device is given in
`deploymentBase`:

| `type` | `download` | `update` | Meaning |
|---|---|---|---|
| `forced` (default) | `forced` | `forced` | install as soon as possible |
| `soft` | `attempt` | `attempt` | the device may defer per its own policy |
| `timeforced` | `attempt` → `forced` | `attempt` → `forced` | soft until `forcetime`, forced after |
| `downloadonly` | `forced` | `skip` | fetch the artifacts, do not install |

An unknown type is rejected with `400`.

`timeforced` takes a `forcetime` (epoch millis) alongside the type:

```bash
curl -u admin:pw -X POST localhost:8088/rest/v1/targets/device-42/assignedDS \
  -H 'Content-Type: application/json' \
  -d '{"id":1,"type":"timeforced","forcetime":1767225600000}'
```

Before that instant the device sees `attempt`; after it, `forced` — no server-side
job is involved, the mode is computed per request. Omitting `forcetime` means
"already reached", so the action behaves as `forced` immediately (matching
hawkBit's default of `0`). Note the request body spells it all-lowercase
`forcetime` while the action response uses `forceTime`; that asymmetry is
hawkBit's and raptor mirrors it.

### Maintenance windows

A maintenance window splits download from install: the device fetches the
artifacts as soon as the action is assigned, but is told to hold the install
until the window opens. Use it when the update itself is disruptive — a vehicle
that must not reboot mid-journey, a machine that may only restart overnight.

```bash
curl -u admin:pw -X POST localhost:8088/rest/v1/targets/device-42/assignedDS \
  -H 'Content-Type: application/json' -d '{
    "id": 1, "type": "forced",
    "maintenanceWindow": {
      "schedule": "0 0 2 ? * MON",
      "duration": "02:00:00",
      "timezone": "+02:00"
    }
  }'
```

That window opens at 02:00 every Monday, in UTC+02:00, and stays open for two
hours.

- **`schedule`** — [Quartz cron][quartz], **not** Unix cron. It leads with a
  seconds field (six fields, or seven with a trailing year), accepts `?` as the
  "no specific value" wildcard in the two day fields, and numbers weekdays
  **1 = Sunday through 7 = Saturday**. A Unix-cron five-field expression is
  rejected rather than silently misread, but a *valid* expression using the
  other weekday numbering is not detectable — `2` means Monday here and Tuesday
  in Unix cron, so double-check day-of-week schedules.
- **`duration`** — how long the window stays open, `HH:mm:ss`, up to `23:59:59`.
- **`timezone`** — offset from UTC as `±HH:mm`. It is a fixed offset, not a
  named zone, so it does not follow daylight-saving transitions: a window set
  at `+01:00` in winter opens an hour early once summer time starts.

While the window is shut the device's `deploymentBase` reports
`update: "skip"` alongside `maintenanceWindow: "unavailable"`, with `download`
left at the action's real mode. Once it opens, `update` becomes the action's
real mode and `maintenanceWindow` reads `"available"`. Nothing is scheduled
server-side — the state is computed per request, the same way `timeforced`
works — so a device simply polls and finds the window open.

The action echoes its window back to operators, with the next opening:

```json
"maintenanceWindow": {
  "schedule": "0 0 2 ? * MON", "duration": "02:00:00",
  "timezone": "+02:00", "nextStartAt": 1767225600000
}
```

A window the server cannot evaluate — a malformed schedule, a duration that is
not `HH:mm:ss`, an offset that is not `±HH:mm`, or a schedule pinned to a year
already past — is rejected with `400` at assignment time, so a device is never
handed a window that would leave it waiting forever.

Windows currently apply to direct assignments only; rollouts and target-filter
auto-assignment do not carry one yet ([#7]).

[quartz]: https://www.quartz-scheduler.org/documentation/quartz-2.3.0/tutorials/crontrigger.html
[#7]: https://github.com/rosterloh/raptor/issues/7

A **`downloadonly`** action completes when the device reports `downloaded`
feedback rather than `closed`. Because nothing was installed, the target's
`installedDS` is deliberately left untouched — only `assignedDS` reflects the
distribution set. The action ends with `status: finished` and
`detailStatus: downloaded`.

### Escalating a running action

A soft or timeforced action can be pushed through immediately:

```bash
curl -u admin:pw -X PUT localhost:8088/rest/v1/targets/device-42/actions/7 \
  -H 'Content-Type: application/json' -d '{"forceType":"forced"}'
```

The next `deploymentBase` the device fetches carries `forced`. Escalating an
action that is no longer active returns `410 Gone`.

## One active action per target

raptor enforces hawkBit's default invariant: **a target has at most one active
action**. Assigning a new DS to a target that already has an active action
cancels the old one and starts the new deployment. (hawkBit's opt-in
multi-assignment mode with action weights is not implemented.)

## Action states

| State | `active` | Meaning |
|---|---|---|
| `wait_for_confirmation` | yes | awaiting confirmation before deploying (see [Confirmation Flow](./confirmation-flow.md)) |
| `running` | yes | device has been told to deploy |
| `canceling` | yes | cancellation requested, awaiting device acknowledgement |
| `canceled` | no | cancellation confirmed (or forced) |
| `finished` | no | deployment succeeded |
| `error` | no | deployment failed |

Each transition and every piece of device feedback appends an **ActionStatus**
history row (with optional messages).

## Inspecting actions

```bash
# all actions on one target (newest first)
curl -u admin:pw localhost:8088/rest/v1/targets/device-42/actions

# a single action
curl -u admin:pw localhost:8088/rest/v1/targets/device-42/actions/1

# fleet-wide, filterable
curl -u admin:pw 'localhost:8088/rest/v1/actions?q=detailStatus==error'
```

The action JSON exposes `status` (`pending` while active, else `finished`) and
`detailStatus` (the fine-grained state from the table above).

## Status history

Every state change an action goes through — assignment, each piece of device
feedback, cancellation — is recorded as a status entry. List them with:

```bash
# chronological (oldest first); pass ?sort=id:DESC for newest first
curl -u admin:pw localhost:8088/rest/v1/targets/device-42/actions/1/status
```

Each entry has a `type` (the reported status, e.g. `running`, `finished`,
`canceled`), any `messages` the device or server attached, and `reportedAt`.
The list supports the usual `offset`/`limit`/`sort` paging.

Two settings bound how much of this accumulates. `[quota]
max_status_entries_per_action` caps how many entries one device may report
against a single action (default 1000), and `[cleanup]` deletes closed actions
and their history past a retention window — off by default. Both are in the
[Configuration Reference](../reference/configuration.md#cleanup--automatic-action-cleanup).

## Cancelling

```bash
# request cancellation (device must acknowledge)
curl -u admin:pw -X DELETE localhost:8088/rest/v1/targets/device-42/actions/1

# force-cancel server-side (no device acknowledgement)
curl -u admin:pw -X DELETE 'localhost:8088/rest/v1/targets/device-42/actions/1?force=true'
```

A normal cancel moves the action to `canceling` and offers the device a
`cancelAction` link; the device confirms via cancel feedback, moving it to
`canceled`. A forced cancel closes the action immediately.

## Installed vs assigned

- `GET /rest/v1/targets/{cid}/assignedDS` — the DS currently assigned (what the
  device *should* run).
- `GET /rest/v1/targets/{cid}/installedDS` — the DS the device last successfully
  installed.

Both return `204 No Content` when there is nothing to report.
