# raptor v1 — hawkBit-compatible OTA update server in Rust

**Date:** 2026-07-04
**Status:** Approved design, pre-implementation

## Purpose

raptor is a Rust reimplementation of Eclipse hawkBit's server, targeting **drop-in API
compatibility**: existing hawkBit device clients (SWUpdate, RAUC hawkbit-updater,
Collabora's `hawkbit` Rust crate) and Management API tooling work unchanged. The
deployment story is one static binary + one config file, replacing hawkBit's
Java + MySQL + RabbitMQ stack.

## v1 Scope

**In:**
- DDI API (device polling, deployment, feedback, artifact download) — hawkBit DDI v1 contract
- Management API core workflow: targets, software modules, distribution sets,
  artifact upload, assignments, actions
- FIQL/RSQL `q=` filtering, hawkBit paging/sorting on all list endpoints
- Auth: per-target security tokens, shared gateway token, anonymous DDI mode,
  HTTP Basic for the Management API
- SQLite **and** Postgres support from day one
- Content-addressed artifact storage on local disk
- Single tenant internally; DDI URLs accept (and ignore) any tenant path segment

**Out (deferred, schema must not preclude):**
- Rollout engine, tags, target types, target filter queries / auto-assignment
- Real multi-tenant isolation
- Web UI, DMF/AMQP
- DDI confirmation flow (`confirmationBase`), maintenance windows
- Write access to software-module / distribution-set types (read-only seeded types in v1)

## Architecture

Modular monolith. Cargo workspace:

```
raptor/
├── Cargo.toml            # workspace
├── raptor/               # main crate (bin + lib)
│   └── src/
│       ├── main.rs           # CLI (serve, hash-password), startup, router assembly
│       ├── config.rs         # TOML + RAPTOR_* env overrides
│       ├── api/
│       │   ├── ddi/          # /{tenant}/controller/v1/... handlers
│       │   └── mgmt/         # /rest/v1/... handlers
│       ├── domain/           # services: targets, modules, distsets, actions, deployment
│       ├── entity/           # SeaORM entities
│       ├── fiql/             # FIQL/RSQL parser → SeaORM Condition
│       ├── storage/          # content-addressed artifact store
│       └── auth/             # tower middleware for both auth zones
└── migration/            # sea-orm-migration crate (one migration set, both DBs)
```

**Stack:** axum, SeaORM (SQLite + Postgres via one entity/migration definition),
tokio, winnow (FIQL parser), sha2/sha1/md-5, tracing, clap, argon2.
Backend selected by `database_url` scheme. Migrations run automatically at startup.

## Domain model

- **Target** — `controller_id` (unique device identity), name, security token,
  `update_status` ∈ {unknown, registered, pending, in_sync, error}, last poll time,
  request-origin address. Key/value `target_attributes` table populated via DDI
  `configData`.
- **SoftwareModule** — name, version, type, vendor; owns artifacts. Seeded types:
  `os`, `firmware`, `runtime`, `application`.
- **Artifact** — filename, size, sha1/md5/sha256; belongs to one software module.
  Blob stored once per sha256 on disk; rows are references.
- **DistributionSet** — name, version, type, `complete`, `required_migration_step`;
  joins software modules. Seeded types: `os`, `os_app`, `app`.
- **Action** — deployment of a DS to a target. `status` ∈ {running, canceling,
  canceled, finished, error}, `active` flag, forced/soft. **Invariant: at most one
  active action per target** — assigning a new DS cancels the running action
  (hawkBit default behavior).
- **ActionStatus** — history rows (+ messages) appended on every device feedback
  and every server-side transition.

### Update lifecycle

1. Operator `POST /rest/v1/targets/{id}/assignedDS` → Action created (running),
   target → `pending`; any prior active action → `canceling`.
2. Device polls DDI root → `_links.deploymentBase` present.
3. Device fetches `deploymentBase/{actionId}`, downloads artifacts, POSTs feedback
   (`proceeding` → `closed`/`success` or `failure`).
4. On success: action → finished, target → `in_sync`, `installedBase` reflects the
   action. On failure: action → error, target → `error`.
5. Cancellation: `_links.cancelAction` served to device; device confirms via cancel
   feedback → action → canceled.

**Auto-registration:** an unknown `controller_id` polling with a valid gateway token
(or in anonymous mode) creates the target with status `registered` (hawkBit
plug-and-play). Polling with an unknown/invalid target token → 401.

## API surface

### DDI — `/{tenant}/controller/v1/{controllerId}`

Tenant segment: accepted, ignored. Response JSON matches hawkBit DDI v1 schemas
field-for-field (verified by golden-fixture tests).

| Endpoint | Behavior |
|---|---|
| `GET /` | Poll root: `config.polling.sleep` (HH:MM:SS) + `_links` (deploymentBase / cancelAction / configData / installedBase as applicable). Updates last poll time. |
| `GET /deploymentBase/{actionId}` | Deployment: download/update mode (forced/attempt/skip), chunks → artifacts with hashes, size, download `_links`. |
| `POST /deploymentBase/{actionId}/feedback` | Progress feedback; drives action state machine; appends ActionStatus. Non-active action → 410 Gone. |
| `GET /cancelAction/{actionId}` | Cancellation payload. |
| `POST /cancelAction/{actionId}/feedback` | Confirms cancellation. |
| `PUT /configData` | Device attributes; modes merge / replace / remove. |
| `GET /installedBase/{actionId}` | Last successfully installed deployment. |
| `GET /softwaremodules/{id}/artifacts` | Artifact list for a module. |
| `GET /softwaremodules/{id}/artifacts/{filename}` | Artifact download. **Supports HTTP Range** (RFC 7233) for resume. |
| `GET /softwaremodules/{id}/artifacts/{filename}.MD5SUM` | md5sum-file format. |

### Management API — `/rest/v1/`

- `targets` — CRUD; `/{id}/assignedDS` (POST = assign/deploy), `/installedDS`,
  `/actions`, `/actions/{id}` (DELETE = cancel), `/attributes`.
- `softwaremodules` — CRUD; `POST /{id}/artifacts` multipart upload (server computes
  sha1/md5/sha256 while streaming); artifact list / download / delete.
- `distributionsets` — CRUD; `/{id}/assignedSM` compose modules.
- `actions` — fleet-wide list/filter.
- `softwaremoduletypes`, `distributionsettypes` — **read-only** (seeded rows).
- List endpoints: `offset`/`limit` paging with `{content, total, size}` envelope,
  `sort=field:ASC|DESC`, `q=` FIQL.

### FIQL

No maintained Rust RSQL crate exists → small `winnow` parser in `fiql/`.
Grammar: comparisons `==`, `!=`, `=lt=`, `=le=`, `=gt=`, `=ge=`, `=in=`, `=out=`;
`*` wildcards in values (→ SQL LIKE); `;` = AND, `,` = OR, `;` binds tighter;
parentheses. Compiles to SeaORM `Condition` against a per-resource field map
(unknown field → 400). Heavy table-driven unit tests.

## Auth

Tower middleware, two zones:

- **DDI zone:** `Authorization: TargetToken <t>` (must match the target's stored
  token) or `Authorization: GatewayToken <t>` (shared token from config; enables
  auto-registration). `ddi.anonymous = true` disables DDI auth (dev mode, default
  off).
- **Mgmt zone:** HTTP Basic against a single admin credential from config
  (argon2id hash; generated with `raptor hash-password`). No user table in v1.

## Artifact storage

- Layout: `<artifact_dir>/<sha256[0..2]>/<sha256>`.
- Upload streams to a temp file computing all three hashes, then renames into place
  (dedup: if blob exists, temp discarded).
- Blob deleted only when the last referencing artifact row is deleted.
- Configurable max upload size.

## Configuration

`raptor.toml`, every key overridable via `RAPTOR_*` env vars:

```toml
bind = "0.0.0.0:8080"
database_url = "sqlite://raptor.db"      # or postgres://user:pass@host/db
artifact_dir = "/var/lib/raptor/artifacts"
max_artifact_size = "1GiB"

[ddi]
anonymous = false
gateway_token = "..."                     # optional
polling_interval = "00:05:00"

[mgmt]
username = "admin"
password_hash = "$argon2id$..."
```

CLI: `raptor serve [--config path]`, `raptor hash-password`.

## Error handling

Single `AppError` → responses matching hawkBit status codes exactly (clients branch
on these), with hawkBit-shaped bodies:
`{"exceptionClass": "...", "errorCode": "hawkbit.server.error.repo.entityNotFound", "message": "..."}`
using hawkBit's real errorCode strings for covered cases. Notable codes:
404 unknown entity; 400 invalid FIQL / malformed body; 401 bad or missing
credentials; 410 feedback for non-active action; 409 duplicate key
(e.g. module name+version+type).

## Testing

1. **Unit:** FIQL parser (table-driven), action state machine transitions,
   hash pipeline.
2. **Integration:** full axum app; per-test in-memory SQLite; identical suite runs
   against Postgres in CI (service container). Golden JSON fixtures for DDI
   responses derived from hawkBit's published API documentation.
3. **E2E compat:** drive Collabora's `hawkbit` DDI client crate through a full
   cycle (register → poll → deploy → download → feedback → in_sync) against a
   running raptor. Manual milestone: real SWUpdate or RAUC device.

## Milestones

1. **Skeleton** — workspace, config, CLI, migrations apply on SQLite + Postgres,
   health endpoint. *Verify: `raptor serve` boots on both DB URLs.*
2. **Modules + artifacts** — softwaremodules CRUD, multipart upload, blob store.
   *Verify: upload via curl, hashes correct, dedup works.*
3. **Targets + distsets + query layer** — CRUD, FIQL, paging, sorting.
   *Verify: integration suite green on both DBs.*
4. **DDI base** — poll root, configData, auto-registration, auth middleware.
   *Verify: golden fixtures match; token matrix tests.*
5. **DDI deployment** — deploymentBase, Range downloads, feedback state machine,
   cancel flow. *Verify: full lifecycle integration test.*
6. **Compat proof** — hawkbit-rs E2E test, error-body polish.
   *Verify: E2E green; manual SWUpdate check.*
