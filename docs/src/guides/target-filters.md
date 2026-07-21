# Target Filters & Auto-Assignment

A **target filter** is a saved FIQL query. On its own it's a convenient, named
way to select devices. Attach a distribution set to it, and it becomes an
**auto-assignment rule**: matching targets receive that DS automatically — now,
and as new matching devices appear.

## Creating a filter

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/targetfilters \
  -H 'Content-Type: application/json' \
  -d '{"name":"beta-ring","query":"controllerId==beta-*"}'
```

The `query` is validated against the target field map at write time — an invalid
FIQL expression returns `400 Bad Request`, and a duplicate name returns `409
Conflict`. Update, fetch, list, and delete follow the usual REST shape:

```bash
curl -u admin:pw -X PUT    localhost:8080/rest/v1/targetfilters/1 \
  -H 'Content-Type: application/json' -d '{"query":"name==beta-*"}'
curl -u admin:pw           localhost:8080/rest/v1/targetfilters
curl -u admin:pw -X DELETE localhost:8080/rest/v1/targetfilters/1
```

## Attaching an auto-assign distribution set

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/targetfilters/1/autoAssignDS \
  -H 'Content-Type: application/json' \
  -d '{"id":1,"type":"forced"}'
```

- `id` — the distribution set to assign. It must be **complete** (`400` otherwise).
- `type` — `forced` (default) or `soft`.

Attaching immediately assigns the DS to every currently-matching target. Read or
detach the attachment with:

```bash
curl -u admin:pw           localhost:8080/rest/v1/targetfilters/1/autoAssignDS   # -> the DS, or 204
curl -u admin:pw -X DELETE localhost:8080/rest/v1/targetfilters/1/autoAssignDS
```

## When auto-assignment runs

A matching target receives the DS:

- **On registration** — a device auto-registering via DDI that matches a filter
  is assigned in the same poll (its very first poll can already return a
  `deploymentBase`).
- **On attribute change** — updating a target via `configData` re-evaluates the
  filters.
- **Periodically** — a background sweep (sharing the rollout evaluator's task)
  catches targets created through other paths.

## Non-disruptive by design

Auto-assignment never disturbs work in flight. A matching target is **skipped** if
it already has that DS assigned, or if it has any active action. So a device
mid-deployment is never clobbered by an auto-assign rule; it's picked up on a
later sweep once it's idle.

> **Note:** FIQL auto-assignment matches on the standard target fields
> (`controllerId`, `name`, `updateStatus`, …). Matching on device-reported
> *attributes* is not yet supported.
