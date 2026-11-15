# Proposal: raptor v1

## Why

Existing hawkBit device clients (SWUpdate, rauc-hawkbit-updater, the Zephyr
client) need a server, and hawkBit's Java + MySQL + RabbitMQ stack is heavy for a
single fleet. raptor v1 is a Rust server that speaks the same APIs, shipped as one
binary and one config file.

## What Changes

- DDI v1 device API: poll root, `deploymentBase`, feedback, `cancelAction`,
  `configData`, `installedBase`, artifact download with HTTP Range and `.MD5SUM`.
- Management API core: targets, software modules, artifacts, distribution sets,
  assignments and actions, with hawkBit paging, sorting and FIQL `q=` filtering.
- Auth: per-target tokens, a shared gateway token, anonymous DDI mode, and HTTP
  Basic for the Management API against one argon2id-hashed admin account.
- SQLite and Postgres from day one; content-addressed artifact storage on disk.
- CLI: `raptor serve` and `raptor hash-password`.

## Capabilities

### New Capabilities

Imported history: this change predates OpenSpec. The behaviour it introduced is
described, as it stands today, by these specs: `ddi-polling`, `ddi-deployment`,
`artifact-download`, `ddi-auth`, `mgmt-auth`, `mgmt-api-conventions`,
`targets`, `software-modules`, `artifact-storage`, `distribution-sets`,
`deployment-actions`, `server-runtime`.

### Modified Capabilities

None.

## Impact

New workspace: `raptor` (bin + lib), `migration`. See `design.md` for the
original design and `plan.md` for the task-by-task plan.
