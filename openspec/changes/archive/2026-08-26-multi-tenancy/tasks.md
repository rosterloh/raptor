# Tasks

Reconstructed from the "What shipped alongside this doc" section of
`design.md`; this change had no separate plan.

## 1. Schema

- [x] 1.1 Add `tenant TEXT NOT NULL DEFAULT 'DEFAULT'` to the 11 query-root tables
- [x] 1.2 Convert inline unique constraints to named composite `(tenant, x)` indexes
- [x] 1.3 Matching entity changes; migration runs on SQLite and Postgres

## 2. DDI tenant guard

- [x] 2.1 Add the `tenant` config key (default `DEFAULT`)
- [x] 2.2 Reject other DDI tenant segments with `404` in `ddi_auth`, case-insensitively

## 3. Docs

- [x] 3.1 Record the decision and the composite-index convention in `CLAUDE.md`
