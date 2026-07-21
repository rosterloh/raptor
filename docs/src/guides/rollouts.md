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

## Evaluator cadence

The background evaluator runs every `rollout_eval_interval_secs` seconds
(default 5). Lower it for snappier progression in testing, raise it to reduce
load on large fleets. See the
[Configuration Reference](../reference/configuration.md).

> **Note:** hawkBit's rollout **approval workflow** and **dynamic rollouts**
> (groups that keep absorbing newly-matching targets) are not yet implemented.
> Group membership is a static snapshot taken at creation time.
