# Management API Reference

Operator-facing REST API under `/rest/v1`, authenticated with HTTP Basic (or a
session cookie). All list endpoints accept `offset`, `limit`, `sort=field:ASC|DESC`,
and `q=<FIQL>`, and return the hawkBit paged envelope `{content, total, size}`.

Base URL examples assume `localhost:8088`.

Handlers live under `raptor/src/api/mgmt/`, one module per resource family,
each exposing a `routes()` that `mod.rs` merges — e.g. target endpoints in
`targets.rs`, distribution-set/software-module/target *type* endpoints in
`types/`.

## Auth & session

| Method | Path | Description |
|---|---|---|
| `POST` | `/rest/v1/login` | exchange credentials for a session cookie |
| `POST` | `/rest/v1/logout` | clear the session |
| `GET` | `/rest/v1/session` | 204 if the request is authenticated, 401 if not |
| `GET` | `/health` | liveness probe (returns `ok`) |

## Targets

| Method | Path | Description |
|---|---|---|
| `POST` | `/rest/v1/targets` | create targets (JSON array) |
| `GET` | `/rest/v1/targets` | list (paging/sort/FIQL) |
| `GET` | `/rest/v1/targets/{cid}` | get one |
| `PUT` | `/rest/v1/targets/{cid}` | update name/description/token/`requestAttributes` |
| `DELETE` | `/rest/v1/targets/{cid}` | delete |
| `GET` | `/rest/v1/targets/{cid}/attributes` | device-reported attributes |
| `POST` / `DELETE` | `/rest/v1/targets/{cid}/targettype` | assign / unassign the target type |
| `POST` / `GET` | `/rest/v1/targets/{cid}/metadata` | create (JSON array) / list metadata |

| `GET` / `PUT` / `DELETE` | `/rest/v1/targets/{cid}/metadata/{key}` | get / update / delete one entry |
| `POST` | `/rest/v1/targets/{cid}/assignedDS` | assign a DS (creates an action) |
| `GET` | `/rest/v1/targets/{cid}/assignedDS` | currently assigned DS (or 204) |
| `GET` | `/rest/v1/targets/{cid}/installedDS` | last installed DS (or 204) |
| `GET` | `/rest/v1/targets/{cid}/actions` | actions for this target |
| `GET` | `/rest/v1/targets/{cid}/actions/{aid}` | one action |
| `PUT` | `/rest/v1/targets/{cid}/actions/{aid}` | escalate the force type, `{"forceType":"forced"}` |
| `GET` | `/rest/v1/targets/{cid}/actions/{aid}/status` | action status history (paging/sort) |
| `DELETE` | `/rest/v1/targets/{cid}/actions/{aid}` | cancel (`?force=true` to force) |
| `GET` | `/rest/v1/targets/{cid}/autoConfirm` | auto-confirm state |
| `POST` | `/rest/v1/targets/{cid}/autoConfirm/activate` | enable auto-confirm |
| `POST` | `/rest/v1/targets/{cid}/autoConfirm/deactivate` | disable auto-confirm |

A target's `requestAttributes` flag controls whether its DDI poll advertises the
`configData` link. It is set when the target is created and cleared once the
device uploads its attributes; re-arm it to ask for a fresh upload:

```bash
curl -u admin:pw -X PUT localhost:8088/rest/v1/targets/device-42 \
  -H 'Content-Type: application/json' -d '{"requestAttributes": true}'
```

### Embedded set and tag references

`GET /rest/v1/targets` and `GET /rest/v1/targets/{cid}` include the target's
installed and assigned distribution sets, and its tags, inline:

```jsonc
{
  "controllerId": "dev-a91f3c",
  "updateStatus": "pending",
  "installedDs": { "id": 4, "name": "gw-linux", "version": "2026.05", "type": "os_app" },
  "assignedDs":  { "id": 7, "name": "gw-linux", "version": "2026.07", "type": "os_app" },
  "tags": [{ "id": 2, "name": "linux-gw", "colour": "#4f9cf9" }]
}
```

A raptor extension, and additive: hawkBit exposes these only as `installedDS`,
`assignedDS` and per-tag reverse lookups, so a list view could otherwise only
fetch them one request per row. The fields are **omitted** when absent — no
installed set is different from an empty one — and every other endpoint that
returns a target (create, tag assignment) serialises exactly as before.

Resolved in a fixed number of queries for the whole page regardless of page size,
because `assigned_ds_id` and `installed_ds_id` are columns on the target row.

The set `type` is the type *key* (`os`, `app`, `os_app`), not its id: it is what
says whether a given device class can install that set at all.

## Software modules & artifacts

| Method | Path | Description |
|---|---|---|
| `POST` / `GET` | `/rest/v1/softwaremodules` | create / list |
| `GET` / `PUT` / `DELETE` | `/rest/v1/softwaremodules/{id}` | get / update / delete |
| `POST` / `GET` | `/rest/v1/softwaremodules/{id}/artifacts` | upload (multipart) / list |
| `GET` / `DELETE` | `/rest/v1/softwaremodules/{id}/artifacts/{aid}` | get / delete |
| `GET` | `/rest/v1/softwaremodules/{id}/artifacts/{aid}/download` | download |
| `POST` / `GET` | `/rest/v1/softwaremodules/{id}/metadata` | create (JSON array) / list metadata |
| `GET` / `PUT` / `DELETE` | `/rest/v1/softwaremodules/{id}/metadata/{key}` | get / update / delete one entry (`targetVisible` surfaces to devices) |

## Distribution sets

| Method | Path | Description |
|---|---|---|
| `POST` / `GET` | `/rest/v1/distributionsets` | create / list |
| `GET` / `PUT` / `DELETE` | `/rest/v1/distributionsets/{id}` | get / update / delete |
| `POST` | `/rest/v1/distributionsets/{id}/invalidate` | invalidate (stops rollouts / auto-assign, cancels actions) |
| `POST` / `GET` | `/rest/v1/distributionsets/{id}/assignedSM` | add / list modules |
| `POST` / `GET` | `/rest/v1/distributionsets/{id}/metadata` | create (JSON array) / list metadata |
| `GET` / `PUT` / `DELETE` | `/rest/v1/distributionsets/{id}/metadata/{key}` | get / update / delete one entry |

## Actions (fleet-wide)

| Method | Path | Description |
|---|---|---|
| `GET` | `/rest/v1/actions` | list all actions (paging/sort/FIQL) |
| `GET` | `/rest/v1/system/configs` | tenant configuration (read-only; file-driven) |
| `GET` / `PUT` / `DELETE` | `/rest/v1/system/configs/{key}` | one config key (writes → 403) |
| `GET` | `/rest/v1/system/statistics` | fleet counters (targets/actions/…), optional `q=` |

### Scoping statistics to a filter

`/rest/v1/system/statistics` accepts an optional FIQL `q=`, the same query the
target list and saved target filters take:

```
curl -u admin:pw 'localhost:8088/rest/v1/system/statistics?q=tag%3D%3Dlinux-gw'
```

`q` narrows the **target** counters — `totalTargets` and `targetsByStatus` — to
the matching targets. It lets a caller read one saved filter's in-sync /
pending / error split in a single request rather than one request per status.

The catalogue and action counters (`totalDistributionSets`,
`totalSoftwareModules`, `totalActions`, `totalRollouts`, `activeActions`) stay
fleet-wide whether or not `q` is set: scoping "how many distribution sets exist"
to a set of targets has no meaning, and reporting them as `0` would read as
missing data. An unparseable or unknown-field query returns `400`.

## Types

Software-module, distribution-set and target types support full CRUD. The
default `os` / `firmware` / `runtime` / `application` module types and `os` /
`os_app` / `app` DS types are seeded, and a DS type's mandatory module types
drive whether a distribution set is `complete`. Deleting a type that is still in
use returns `409`.

| Method | Path | Description |
|---|---|---|
| `POST` / `GET` | `/rest/v1/softwaremoduletypes` | create / list module types |
| `GET` / `PUT` / `DELETE` | `/rest/v1/softwaremoduletypes/{id}` | get / update (description) / delete |
| `POST` / `GET` | `/rest/v1/distributionsettypes` | create (with mandatory/optional module types) / list |
| `GET` / `PUT` / `DELETE` | `/rest/v1/distributionsettypes/{id}` | get / update (description) / delete |
| `GET` / `POST` | `/rest/v1/distributionsettypes/{id}/mandatorymoduletypes` | list / add mandatory module type |
| `DELETE` | `/rest/v1/distributionsettypes/{id}/mandatorymoduletypes/{mid}` | remove mandatory module type |
| `GET` / `POST` | `/rest/v1/distributionsettypes/{id}/optionalmoduletypes` | list / add optional module type |
| `DELETE` | `/rest/v1/distributionsettypes/{id}/optionalmoduletypes/{mid}` | remove optional module type |
| `POST` / `GET` | `/rest/v1/targettypes` | create (with compatible DS types) / list |
| `GET` / `PUT` / `DELETE` | `/rest/v1/targettypes/{id}` | get / update / delete |
| `GET` / `POST` | `/rest/v1/targettypes/{id}/compatibledistributionsettypes` | list / add compatible DS type |
| `DELETE` | `/rest/v1/targettypes/{id}/compatibledistributionsettypes/{dsid}` | remove compatible DS type |

## Tags

Target and distribution-set tags are free-form labels (name, description,
`colour`) used for fleet organisation and as the `tag==` FIQL term on the target
and distribution-set lists. Deleting a tag removes its assignments; the tagged
targets and sets are untouched. Assignment is idempotent — re-assigning an
already-tagged entity succeeds without creating a duplicate.

| Method | Path | Description |
|---|---|---|
| `POST` / `GET` | `/rest/v1/targettags` | create (array body) / list |
| `GET` / `PUT` / `DELETE` | `/rest/v1/targettags/{id}` | get / update / delete |
| `GET` | `/rest/v1/targettags/{id}/assigned` | list tagged targets (paging, `sort=`, `q=`) |
| `GET` | `/rest/v1/targets/{cid}/tags` | tags carried by one target (raptor extension) |
| `POST` / `DELETE` | `/rest/v1/targettags/{id}/assigned` | bulk assign / unassign, body `["dev-1","dev-2"]` |
| `POST` / `DELETE` | `/rest/v1/targettags/{id}/assigned/{cid}` | assign / unassign one target |
| `POST` / `GET` | `/rest/v1/distributionsettags` | create (array body) / list |
| `GET` / `PUT` / `DELETE` | `/rest/v1/distributionsettags/{id}` | get / update / delete |
| `GET` | `/rest/v1/distributionsettags/{id}/assigned` | list tagged distribution sets |
| `GET` | `/rest/v1/distributionsets/{id}/tags` | tags carried by one set (raptor extension) |
| `POST` / `DELETE` | `/rest/v1/distributionsettags/{id}/assigned` | bulk assign / unassign, body `[1,2]` |
| `POST` / `DELETE` | `/rest/v1/distributionsettags/{id}/assigned/{dsid}` | assign / unassign one set |

## Rollouts

| Method | Path | Description |
|---|---|---|
| `POST` / `GET` | `/rest/v1/rollouts` | create / list |
| `GET` / `DELETE` | `/rest/v1/rollouts/{id}` | get / delete |
| `POST` | `/rest/v1/rollouts/{id}/start` | start (schedules first group) |
| `POST` | `/rest/v1/rollouts/{id}/pause` | pause |
| `POST` | `/rest/v1/rollouts/{id}/resume` | resume |
| `GET` | `/rest/v1/rollouts/{id}/deploygroups` | list groups |
| `GET` | `/rest/v1/rollouts/{id}/deploygroups/{gid}` | one group |
| `GET` | `/rest/v1/rollouts/{id}/deploygroups/{gid}/targets` | controllerIds in a group |

Rollout and group payloads carry `totalTargetsPerStatus` (notstarted, scheduled,
running, error, finished, cancelled) — see the
[Rollouts guide](../guides/rollouts.md#tracking-progress).

## Target filters

| Method | Path | Description |
|---|---|---|
| `POST` / `GET` | `/rest/v1/targetfilters` | create / list |
| `GET` / `PUT` / `DELETE` | `/rest/v1/targetfilters/{id}` | get / update / delete |
| `GET` / `POST` / `DELETE` | `/rest/v1/targetfilters/{id}/autoAssignDS` | read / attach / detach auto-assign DS |

## Common status codes

| Code | When |
|---|---|
| `200` / `201` | success / created |
| `204` | no content (e.g. no assigned DS) |
| `400` | invalid FIQL or malformed body |
| `401` | bad or missing credentials |
| `404` | unknown entity |
| `409` | duplicate key (e.g. module name+version+type) |
| `410` | feedback for a non-active action |

See [Error Codes](./errors.md) for the response body shape.
