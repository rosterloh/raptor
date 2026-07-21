# Assignments & Actions

An **action** is the record of one deployment: a distribution set being rolled
out to one target. Assigning a DS creates an action; the device's feedback drives
it to completion.

## Assigning a distribution set

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/targets/device-42/assignedDS \
  -H 'Content-Type: application/json' -d '{"id":1,"type":"forced"}'
```

The `type` is the action type. raptor supports:

- **`forced`** (default) — the device should install as soon as it can.
- **`soft`** — the device may defer according to its own policy.

> **Note:** hawkBit's `downloadonly` and `timeforced` action types are not yet
> implemented. Any type other than `soft` is treated as `forced`.

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
curl -u admin:pw localhost:8080/rest/v1/targets/device-42/actions

# a single action
curl -u admin:pw localhost:8080/rest/v1/targets/device-42/actions/1

# fleet-wide, filterable
curl -u admin:pw 'localhost:8080/rest/v1/actions?q=detailStatus==error'
```

The action JSON exposes `status` (`pending` while active, else `finished`) and
`detailStatus` (the fine-grained state from the table above).

## Status history

Every state change an action goes through — assignment, each piece of device
feedback, cancellation — is recorded as a status entry. List them with:

```bash
# chronological (oldest first); pass ?sort=id:DESC for newest first
curl -u admin:pw localhost:8080/rest/v1/targets/device-42/actions/1/status
```

Each entry has a `type` (the reported status, e.g. `running`, `finished`,
`canceled`), any `messages` the device or server attached, and `reportedAt`.
The list supports the usual `offset`/`limit`/`sort` paging.

## Cancelling

```bash
# request cancellation (device must acknowledge)
curl -u admin:pw -X DELETE localhost:8080/rest/v1/targets/device-42/actions/1

# force-cancel server-side (no device acknowledgement)
curl -u admin:pw -X DELETE 'localhost:8080/rest/v1/targets/device-42/actions/1?force=true'
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
