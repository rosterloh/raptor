# Proposal: multi-tenancy schema prep (#12)

## Why

hawkBit is multi-tenant; raptor accepted any DDI tenant segment and folded all
devices into one fleet, so two devices with the same `controllerId` under
different tenants collided. Unique constraints declared inline cannot be widened
to `(tenant, x)` on SQLite without a table rebuild, and every new uniquely-named
table makes that more expensive.

## What Changes

- `tenant` column (default `DEFAULT`) on the 11 query-root tables, with unique
  names converted to named composite indexes including `tenant`.
- **BREAKING:** raptor answers to exactly one DDI tenant (new `tenant` config
  key, default `DEFAULT`, case-insensitive) and returns `404` for any other.
- Real per-tenant isolation stays deferred; the Management API half is blocked
  on a user model (#13).

## Capabilities

### New Capabilities

Imported history: described today by `tenancy`.

### Modified Capabilities

None.

## Impact

Migration `m20260826_000001_tenant`; `ddi_auth` middleware; new convention in
`CLAUDE.md` for named composite unique indexes. See `design.md`.
