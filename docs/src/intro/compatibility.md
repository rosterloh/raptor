# hawkBit Compatibility

raptor implements a subset of hawkBit, chosen to cover the device update
workflow first. This page is the source of truth for what exists today.

> **Baseline:** this matrix is measured against hawkBit **1.x**. hawkBit
> reached [1.0 in April
> 2026](https://newsroom.eclipse.org/news/community-news/eclipse-hawkbit%E2%84%A2-10-release)
> and 1.1.0 in July, [removing some features along the
> way](https://github.com/eclipse-hawkbit/hawkbit/releases) — rows below
> reflect that, not the older 0.x contract.

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
| Auto-registration (gateway token / anonymous¹) | ✅ |
| `confirmationBase` confirmation flow | ✅ |
| Maintenance windows | ❌ ([#7](https://github.com/rosterloh/raptor/issues/7)) |
| DMF (AMQP) device path | ❌ ([#11](https://github.com/rosterloh/raptor/issues/11)) |
| Per-target polling interval override | ❌ ([#91](https://github.com/rosterloh/raptor/issues/91)) |

¹ hawkBit 0.8 removed anonymous controller support and anonymous download.
raptor keeps anonymous mode as a **raptor extension** (useful for dev/lab
setups), not a hawkBit 1.x compatibility item — see the [Auth](#auth) table.

## Management API

| Area | Status |
|---|---|
| Targets CRUD, `assignedDS`, `installedDS`, `actions`, `attributes` | ✅ |
| Software modules CRUD + multipart artifact upload/list/download/delete | ✅ |
| Distribution sets CRUD + module composition | ✅ |
| Actions (per-target and fleet-wide list/filter) | ✅ |
| Rollouts (create/start/pause/resume/delete, deploy groups) | ✅ |
| Rollout stop | ❌ ([#90](https://github.com/rosterloh/raptor/issues/90)) |
| Target filters + auto-assignment | ✅ |
| Per-target auto-confirm | ✅ |
| FIQL filter targets by auto-confirm status | ❌ ([#92](https://github.com/rosterloh/raptor/issues/92)) |
| Target groups (`group` attribute, `q=group==`) | ❌ ([#89](https://github.com/rosterloh/raptor/issues/89)) |
| Paging (`offset`/`limit`), `sort=`, `q=` FIQL on lists | ✅ |
| Software-module / distribution-set / target **types** CRUD (composition drives `complete`; target-type/DS-type compatibility enforced) | ✅ |
| Target / distribution-set **tags** CRUD, assign/unassign, `q=tag==x` | ✅ |
| Metadata endpoints (targets / modules / DS, `targetVisible` on module entries) | ✅ |
| All four action types + force escalation (`PUT .../actions/{id}`) and force-quit (`DELETE ...?force=true`) | ✅ |
| Rollout approval workflow ([#17](https://github.com/rosterloh/raptor/issues/17)), dynamic rollouts ([#18](https://github.com/rosterloh/raptor/issues/18)) | ❌ |
| Maintenance windows | ❌ ([#7](https://github.com/rosterloh/raptor/issues/7)) |
| Multi-assignment / action weights | removed upstream in hawkBit 0.10; not planned ([#10](https://github.com/rosterloh/raptor/issues/10)) |

**Wire-format alignment with hawkBit 0.10:** successful deletes return `204 No
Content` — raptor's mgmt delete handlers already do. Quota violations return
`429` upstream; raptor has no quotas yet ([#14](https://github.com/rosterloh/raptor/issues/14)),
which should adopt the same `429` semantics when it lands.

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
| DDI anonymous mode (raptor extension¹) | ✅ |
| Management API HTTP Basic (single admin) | ✅ |
| Session cookie for the web console | ✅ |
| mTLS / certificate DDI auth | ❌ ([#13](https://github.com/rosterloh/raptor/issues/13)) |
| Multiple users / roles, OIDC | ❌ ([#13](https://github.com/rosterloh/raptor/issues/13)) |

## Tenancy

raptor is **single-tenant** ([#12](https://github.com/rosterloh/raptor/issues/12)).
The DDI URL's `/{tenant}/controller/v1/...` segment is accepted and ignored;
all generated links use the tenant name `DEFAULT`. There is no per-tenant data
isolation — run one raptor instance per fleet.

> **Note:** Items marked ❌ link to their tracking issue on the [GitHub
> repository](https://github.com/rosterloh/raptor/issues). The schema is
> designed so these can be added without breaking existing deployments.
