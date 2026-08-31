# Rollouts

A **rollout** deploys a distribution set across many targets in stages, advancing
from one group to the next only when success thresholds are met — so a bad update
is caught on a small group before it reaches the whole fleet.

## How it works

1. You create a rollout from a **FIQL target filter**, a distribution set, and a
   number of groups. Matching targets are split evenly across the groups at
   creation time.
2. Each group has a **success threshold** and an **error threshold** (percentages).
3. Starting the rollout schedules the **first** group only — its targets get the
   DS assigned.
4. A background evaluator watches each running group:
   - When the error threshold is reached, the group and rollout **pause**.
   - When the success threshold is reached, the group **finishes** and the next
     group is scheduled.
5. When the last group finishes, the rollout is **finished**.

## Creating a rollout

```bash
curl -u admin:pw -X POST localhost:8088/rest/v1/rollouts \
  -H 'Content-Type: application/json' \
  -d '{
        "name": "fleet-1.1",
        "distributionSetId": 1,
        "targetFilterQuery": "controllerId==device-*",
        "amountGroups": 3,
        "successCondition": {"condition":"THRESHOLD","expression":"90"},
        "errorCondition":   {"condition":"THRESHOLD","expression":"20"}
      }'
```

- `amountGroups` splits matching targets into that many groups.
- `successCondition.expression` / `errorCondition.expression` are percentages
  (0–100). If `errorCondition` is omitted, the error threshold never trips.
- `type` is the [action type](./actions.md) every action the rollout creates
  inherits — `forced` (default), `soft`, `timeforced` or `downloadonly` — with
  `forcetime` alongside it for `timeforced`. A staged download-then-install is
  therefore a `downloadonly` rollout followed by a `forced` one over the same
  filter. An unknown type is rejected with `400`.

The rollout starts in `ready` — or in `waiting_for_approval` when the
[approval gate](#approval-workflow) is on.

## Lifecycle operations

```bash
curl -u admin:pw -X POST localhost:8088/rest/v1/rollouts/1/start
curl -u admin:pw -X POST localhost:8088/rest/v1/rollouts/1/pause
curl -u admin:pw -X POST localhost:8088/rest/v1/rollouts/1/resume
curl -u admin:pw -X POST localhost:8088/rest/v1/rollouts/1/stop
curl -u admin:pw -X DELETE localhost:8088/rest/v1/rollouts/1
```

- **start** — `ready` → `running`; schedules the first group.
- **pause** — `running` → `paused`; the evaluator ignores paused rollouts.
- **resume** — `paused` → `running`; re-evaluates immediately.
- **stop** — `running` or `paused` → `stopping` → `stopped`; see below.
- **delete** — cancels any active actions in the rollout and removes it.

### Stopping a rollout

Pause only stops raptor from scheduling *more* groups — the updates already sent
out keep running on the devices that have them. Stop is the abort: it cancels
those in-flight updates as well, and is terminal (a stopped rollout cannot be
resumed; create a new one).

The cancellation is soft, so devices are told rather than cut off:

1. Every active action the rollout issued moves to `canceling` and is served to
   the device as `cancelAction` on its next poll. Groups that had not finished
   are marked `stopped`; ones that already finished keep their outcome.
2. The rollout reports `stopping` while those cancels are outstanding.
3. As each device acknowledges over DDI, its action becomes `canceled`. Once
   none are left active the rollout settles to `stopped`.

A rollout with nothing left in flight — every device it reached is already
finished — goes straight to `stopped`.

A device that is offline holds the rollout in `stopping` until it polls again.
That is the honest state: the update has not been called off out in the fleet
yet. To close one out without waiting, force-cancel its action
(`DELETE /rest/v1/targets/{cid}/actions/{aid}?force=true`).

Stop is rejected with `400` from any other status, including a second stop.

## Approval workflow

By default a rollout is created `ready` and an operator can start it straight
away. Set `rollout_approval_enabled = true` to put a second pair of eyes in
front of that:

```toml
rollout_approval_enabled = true
```

A rollout created with the gate on lands in `waiting_for_approval` instead.
`start` on it is refused until someone decides:

```bash
# Approve — the rollout moves to `ready` and can now be started.
curl -u admin:pw -X POST \
  "localhost:8088/rest/v1/rollouts/1/approve?remark=checked+with+ops"

# Or deny it, permanently.
curl -u admin:pw -X POST \
  "localhost:8088/rest/v1/rollouts/1/deny?remark=fleet+is+frozen"
```

Both take an optional `remark` query parameter and answer `204 No Content`, so
re-read the rollout to see the outcome. The decision is reported on the rollout
as `approveDecidedBy` and `approvalRemark` — the asymmetric spelling is
hawkBit's own, and raptor matches it.

Denial is terminal: `approval_denied` is not a startable status and nothing
transitions out of it, so a denied rollout can only be deleted. There is no
"undeny" — create a fresh rollout instead. Because raptor authenticates a
single operator account, `approveDecidedBy` is always that account's username.

The flag is reported to clients as hawkBit's `rollout.approval.enabled` tenant
config key on `GET /rest/v1/system/configs`.

## Inspecting groups

```bash
# deploy groups with per-group status and target counts
curl -u admin:pw localhost:8088/rest/v1/rollouts/1/deploygroups

# one group
curl -u admin:pw localhost:8088/rest/v1/rollouts/1/deploygroups/5

# the controllerIds in a group
curl -u admin:pw localhost:8088/rest/v1/rollouts/1/deploygroups/5/targets
```

## Tracking progress

Rollouts and groups both carry `totalTargetsPerStatus`, hawkBit's breakdown of
their targets by deployment outcome:

```json
{
  "id": 1, "name": "fleet-1.1", "status": "running", "totalTargets": 9,
  "totalTargetsPerStatus": {
    "notstarted": 0, "scheduled": 3, "running": 3,
    "error": 1, "finished": 2, "cancelled": 0
  }
}
```

- `notstarted` — the rollout has not been started, so nothing is deployed yet.
- `scheduled` — the group is waiting its turn; raptor creates actions only when a
  group is scheduled, so these targets have no action yet.
- `running` — an action is in flight (including `canceling` and, with the
  confirmation flow on, `wait_for_confirmation`).
- `finished` / `error` / `cancelled` — the action's terminal state.

A rollout's counts are the sum of its groups'. The web console renders both as
progress bars — see the [Web Console guide](web-console.md).

## Evaluator cadence

The background evaluator runs every `rollout_eval_interval_secs` seconds
(default 5). Lower it for snappier progression in testing, raise it to reduce
load on large fleets. See the
[Configuration Reference](../reference/configuration.md).

> **Note:** hawkBit's **dynamic rollouts** (groups that keep absorbing
> newly-matching targets) are not yet implemented. Group membership is a static
> snapshot taken at creation time.
