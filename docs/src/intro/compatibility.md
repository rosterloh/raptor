# hawkBit Compatibility

raptor implements a subset of hawkBit, chosen to cover the device update
workflow first. This page is the source of truth for what exists today.

## Device API (DDI v1)

The DDI v1 contract is implemented field-for-field and verified with golden
fixtures and an end-to-end test against a real hawkBit DDI client.

| Feature | Status |
|---|---|
| Poll root, `config.polling.sleep`, `_links` | ✅ |
| `deploymentBase` (download/update modes, chunks, artifacts) | ✅ |
| Deployment feedback state machine | ✅ |
| `cancelAction` + cancel feedback | ✅ |
| `configData` (attributes: merge / replace / remove) | ✅ |
| `installedBase` | ✅ |
| Artifact download with **HTTP Range** (resume) | ✅ |
| `.MD5SUM` companion endpoint | ✅ |
| Auto-registration (gateway token / anonymous) | ✅ |
| `confirmationBase` confirmation flow | ✅ |
| Maintenance windows | ❌ |
| DMF (AMQP) device path | ❌ |

## Management API

| Area | Status |
|---|---|
| Targets CRUD, `assignedDS`, `installedDS`, `actions`, `attributes` | ✅ |
| Software modules CRUD + multipart artifact upload/list/download/delete | ✅ |
| Distribution sets CRUD + module composition | ✅ |
| Actions (per-target and fleet-wide list/filter) | ✅ |
| Rollouts (create/start/pause/resume/delete, deploy groups) | ✅ |
| Target filters + auto-assignment | ✅ |
| Per-target auto-confirm | ✅ |
| Paging (`offset`/`limit`), `sort=`, `q=` FIQL on lists | ✅ |
| Software-module / distribution-set / target **types** CRUD (composition drives `complete`; target-type/DS-type compatibility enforced) | ✅ |
| Target / distribution-set **tags** CRUD, assign/unassign, `q=tag==x` | ✅ |
| Metadata endpoints (targets / modules / DS, `targetVisible` on module entries) | ✅ |
| All four action types + force escalation (`PUT .../actions/{id}`) and force-quit (`DELETE ...?force=true`) | ✅ |
| Rollout approval workflow, dynamic rollouts | ❌ |
| Maintenance windows | ❌ |
| Multi-assignment / action weights | ❌ |

## Action types

hawkBit has `forced`, `soft`, `downloadonly`, and `timeforced`. raptor models all
four. `timeforced` starts out soft and escalates once its deadline passes;
`downloadonly` forces the download and never asks the device to install. An
operator can escalate a running action to `forced` with
`PUT /rest/v1/targets/{controllerId}/actions/{actionId}`, and force-quit one that
the device is not acknowledging with `DELETE …/actions/{actionId}?force=true`.

## Auth

| Mechanism | Status |
|---|---|
| DDI target security token | ✅ |
| DDI shared gateway token | ✅ |
| DDI anonymous mode | ✅ |
| Management API HTTP Basic (single admin) | ✅ |
| Session cookie for the web console | ✅ |
| mTLS / certificate DDI auth | ❌ |
| Multiple users / roles, OIDC | ❌ |

## Tenancy

raptor is **single-tenant**. The DDI URL's `/{tenant}/controller/v1/...` segment
is accepted and ignored; all generated links use the tenant name `DEFAULT`.
There is no per-tenant data isolation — run one raptor instance per fleet.

> **Note:** Items marked ❌ are tracked as issues on the
> [GitHub repository](https://github.com/rosterloh/raptor/issues). The schema is
> designed so these can be added without breaking existing deployments.
