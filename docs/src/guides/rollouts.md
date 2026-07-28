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
curl -u admin:pw -X POST localhost:8080/rest/v1/rollouts \
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

The rollout starts in `ready`.

## Lifecycle operations

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/rollouts/1/start
curl -u admin:pw -X POST localhost:8080/rest/v1/rollouts/1/pause
curl -u admin:pw -X POST localhost:8080/rest/v1/rollouts/1/resume
curl -u admin:pw -X DELETE localhost:8080/rest/v1/rollouts/1
```

- **start** — `ready` → `running`; schedules the first group.
- **pause** — `running` → `paused`; the evaluator ignores paused rollouts.
- **resume** — `paused` → `running`; re-evaluates immediately.
- **delete** — cancels any active actions in the rollout and removes it.

## Inspecting groups

```bash
# deploy groups with per-group status and target counts
curl -u admin:pw localhost:8080/rest/v1/rollouts/1/deploygroups

# one group
curl -u admin:pw localhost:8080/rest/v1/rollouts/1/deploygroups/5

# the controllerIds in a group
curl -u admin:pw localhost:8080/rest/v1/rollouts/1/deploygroups/5/targets
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

> **Note:** hawkBit's rollout **approval workflow** and **dynamic rollouts**
> (groups that keep absorbing newly-matching targets) are not yet implemented.
> Group membership is a static snapshot taken at creation time.
