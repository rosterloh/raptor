# Management API Reference

Operator-facing REST API under `/rest/v1`, authenticated with HTTP Basic (or a
session cookie). All list endpoints accept `offset`, `limit`, `sort=field:ASC|DESC`,
and `q=<FIQL>`, and return the hawkBit paged envelope `{content, total, size}`.

Base URL examples assume `localhost:8080`.

## Auth & session

| Method | Path | Description |
|---|---|---|
| `POST` | `/rest/v1/login` | exchange credentials for a session cookie |
| `POST` | `/rest/v1/logout` | clear the session |
| `GET` | `/health` | liveness probe (returns `ok`) |

## Targets

| Method | Path | Description |
|---|---|---|
| `POST` | `/rest/v1/targets` | create targets (JSON array) |
| `GET` | `/rest/v1/targets` | list (paging/sort/FIQL) |
| `GET` | `/rest/v1/targets/{cid}` | get one |
| `PUT` | `/rest/v1/targets/{cid}` | update name/description/token |
| `DELETE` | `/rest/v1/targets/{cid}` | delete |
| `GET` | `/rest/v1/targets/{cid}/attributes` | device-reported attributes |
| `POST` | `/rest/v1/targets/{cid}/assignedDS` | assign a DS (creates an action) |
| `GET` | `/rest/v1/targets/{cid}/assignedDS` | currently assigned DS (or 204) |
| `GET` | `/rest/v1/targets/{cid}/installedDS` | last installed DS (or 204) |
| `GET` | `/rest/v1/targets/{cid}/actions` | actions for this target |
| `GET` | `/rest/v1/targets/{cid}/actions/{aid}` | one action |
| `GET` | `/rest/v1/targets/{cid}/actions/{aid}/status` | action status history (paging/sort) |
| `DELETE` | `/rest/v1/targets/{cid}/actions/{aid}` | cancel (`?force=true` to force) |
| `GET` | `/rest/v1/targets/{cid}/autoConfirm` | auto-confirm state |
| `POST` | `/rest/v1/targets/{cid}/autoConfirm/activate` | enable auto-confirm |
| `POST` | `/rest/v1/targets/{cid}/autoConfirm/deactivate` | disable auto-confirm |

## Software modules & artifacts

| Method | Path | Description |
|---|---|---|
| `POST` / `GET` | `/rest/v1/softwaremodules` | create / list |
| `GET` / `PUT` / `DELETE` | `/rest/v1/softwaremodules/{id}` | get / update / delete |
| `POST` / `GET` | `/rest/v1/softwaremodules/{id}/artifacts` | upload (multipart) / list |
| `GET` / `DELETE` | `/rest/v1/softwaremodules/{id}/artifacts/{aid}` | get / delete |
| `GET` | `/rest/v1/softwaremodules/{id}/artifacts/{aid}/download` | download |

## Distribution sets

| Method | Path | Description |
|---|---|---|
| `POST` / `GET` | `/rest/v1/distributionsets` | create / list |
| `GET` / `PUT` / `DELETE` | `/rest/v1/distributionsets/{id}` | get / update / delete |
| `POST` | `/rest/v1/distributionsets/{id}/invalidate` | invalidate (stops rollouts / auto-assign, cancels actions) |
| `POST` / `GET` | `/rest/v1/distributionsets/{id}/assignedSM` | add / list modules |

## Actions (fleet-wide)

| Method | Path | Description |
|---|---|---|
| `GET` | `/rest/v1/actions` | list all actions (paging/sort/FIQL) |
| `GET` | `/rest/v1/system/configs` | tenant configuration (read-only; file-driven) |
| `GET` / `PUT` / `DELETE` | `/rest/v1/system/configs/{key}` | one config key (writes → 403) |
| `GET` | `/rest/v1/system/statistics` | fleet counters (targets/actions/…) |

## Types (read-only)

| Method | Path | Description |
|---|---|---|
| `GET` | `/rest/v1/softwaremoduletypes` `[/ {id}]` | seeded module types |
| `GET` | `/rest/v1/distributionsettypes` `[/ {id}]` | seeded DS types |

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
