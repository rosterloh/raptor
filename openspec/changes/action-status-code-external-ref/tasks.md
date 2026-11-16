# Tasks

## 1. Schema

- [x] 1.1 Migration: `action.last_status_code`, `action.external_ref`, `action_status.code`; entity fields

## 2. Status codes (#216)

- [x] 2.1 DDI deployment and cancel feedback parse `status.code`; stored on the status entry and as `last_status_code`
- [x] 2.2 `lastStatusCode` on `ActionRest`, `code` on `ActionStatusRest`; FIQL `lastStatusCode`
- [x] 2.3 Console history, `raptorctl action status` and TUI history show the code; TestBackend test

## 3. externalRef (#212)

- [x] 3.1 `externalRef` on `DsAssignment`, stored by `assign_ds`, 512-char limit; `externalRef` on `ActionRest`; FIQL `externalRef`

## 4. Attribute limits (#210)

- [x] 4.1 `configData` rejects keys or values over 128 characters with `400`

## 5. Tests and docs

- [x] 5.1 DTO round-trip tests; integration tests in `ddi_feedback`, `mgmt_assignment`, `ddi_root`
- [x] 5.2 DDI and Management API references, compatibility table, `CHANGELOG.md`
