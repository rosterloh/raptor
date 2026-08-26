# Multi-tenancy — design decision

**Date:** 2026-08-26
**Status:** Approved design, partially implemented (schema only; see below)
**Tracking:** [#12](https://github.com/rosterloh/raptor/issues/12)

## Purpose

hawkBit is multi-tenant. raptor is not: the DDI URL's `/{tenant}/controller/v1/...`
segment exists in the wire contract but every row in the database is implicitly
one fleet. This doc records the decision on whether/how to close that gap, so
the answer doesn't have to be re-derived every time the question comes up
(it has, several times — see `docs/src/faq.md`, `intro/compatibility.md`,
`intro/what-is-raptor.md`, `concepts/domain-model.md`, `guides/zephyr.md`).

## hawkBit's actual tenancy surface

Before weighing options, it's worth being precise about where tenancy actually
lives on the wire, because it's smaller than it looks:

- **DDI:** the tenant is a **path segment only**. No DDI JSON response —
  poll root, deployment base, feedback, artifact metadata — carries a
  `tenant` field. hawkBit 404s a request for an unconfigured tenant; a
  configured client always sees its own tenant reflected back in `_links`.
- **Management API:** hawkBit has **no tenant in the URL at all**.
  `/rest/v1/targets` is implicitly the authenticated user's tenant — mgmt-side
  tenancy is derived from the security context, never the path. There is no
  compatibility gap to close here by URL shape; a tenant-scoped mgmt API would
  need per-tenant auth, which routes through user-model work (#13), not
  through anything in the Management API's request/response shapes.
- **DMF (AMQP, #11, not yet implemented):** carries `tenant` as a **message
  header on every message**. This is the one wire surface where the tenancy
  decision is load-bearing for a not-yet-built feature — #11 should read this
  doc before choosing whether that header is a hardcoded `DEFAULT` or a real
  routing key.

Conclusion: **the tenancy decision is near-neutral to hawkBit wire
compatibility.** It's a data-model and operations decision, not a protocol
one. What it *does* affect: whether raptor is more permissive than hawkBit at
the DDI layer (see below), and the cost of eventually adding real isolation.

## The concrete risk in today's behavior

raptor's DDI middleware accepts *any* tenant segment and folds it into the one
fleet, logging a one-time warning. `get_or_register`
(`raptor/src/api/ddi/root.rs`) looks up an incoming poll by `controller_id`
alone. Consequence: **two devices sharing a `controllerId` under different
hawkBit tenants become one target** if pointed at the same raptor instance —
silent data corruption, not a visible error. This is the sharpest edge in the
current design and is closed independently of the broader decision below (see
"What shipped").

## Options considered

1. **Instance-per-tenant (ratify today's behavior).** One raptor process + DB
   + artifact dir per tenant; make it explicit rather than accidental.
   Zero schema cost. Matches raptor's stated positioning as a lightweight
   single-fleet server (`docs/src/intro/what-is-raptor.md`) and matches
   hawkBit's own common deployment shape (most hawkBit installs run
   single-tenant with tenant `DEFAULT`).
2. **Real row-level isolation.** `tenant` on every table, tenant-scoped
   queries at ~24 call sites, per-tenant gateway tokens, a tenant table
   replacing file-driven config, tenant-scoped mgmt auth. Large: the mgmt half
   is fully blocked on #13 (there is currently one operator account, in
   `raptor.toml`, with no user table to attach a tenant to), and there's no
   compile-time guard against a forgotten `.filter(tenant)` — a missed one is
   a cross-tenant leak, not a compile error.
3. **Schema-prep now, behavior later.** Add `tenant` and convert the affected
   unique constraints to composite `(tenant, x)` keys, while keeping every
   query, every auth path, and every config value exactly as single-tenant as
   today. No user-visible change.

## Decision: option 3, plus closing the DDI collision hazard

The deciding factor was **not** compatibility (it's neutral) but **migration
cost asymmetry**. Eight tables declare their unique name *inline* on
`CREATE TABLE` (`unique_key()` in sea-orm-migration terms — `target.controller_id`,
`rollout.name`, `target_filter.name`, `target_type.name`, `ds_tag.name`,
`target_tag.name`, and both type tables' `key`). Postgres and SQLite both
implement that as a table constraint rather than a droppable index; SQLite in
particular has no `ALTER TABLE ... DROP CONSTRAINT` at all. Converting one of
these to a composite `(tenant, x)` key later means a full table rebuild
(create-new, copy, drop, rename) on SQLite. That cost is fixed today at eight
tables and grows every time a future feature adds another uniquely-named
entity — three landed in the three commits immediately before this one
(target groups, maintenance windows, rollout stop). Doing it now, while the
schema is still this size, is strictly cheaper than doing it whenever real
isolation is eventually justified.

Real isolation (option 2) is *not* being built now. It stays deferred, and
explicitly blocked on #13's user model for the mgmt half. When it is pursued,
the pattern to follow at each of the ~24 read/write call sites is: add
`.filter(entity::Column::Tenant.eq(current_tenant))`, sourced from mgmt
session/auth context (once #13 exists) and from the DDI tenant guard (already
in place — see below) for device-facing queries. Postgres row-level security
could backstop this on that one backend, but SQLite has no RLS and raptor
supports both per `CLAUDE.md`, so the app-level predicate is required
regardless and RLS would only ever be a Postgres-only belt-and-braces layer,
not the mechanism.

## What shipped alongside this doc

Two independent, decoupled changes:

1. **Schema:** `tenant TEXT NOT NULL DEFAULT 'DEFAULT'` added to the 11
   query-root tables (the 8 above, plus `software_module`, `distribution_set`
   — already named-index tables, just widened — and `action`, a fleet-wide
   query root with no unique constraint of its own). Child/join tables
   (`action_status`, `*_metadata`, `*_tag_assignment`, `rollout_group`, …)
   are untouched — they scope through their FK parent, so adding `tenant`
   there would be denormalization with no reader. `artifact` is deliberately
   excluded: the store is content-addressed (`raptor/src/storage.rs`), blob
   hashes are tenant-neutral, and refcount-by-hash is already correct across
   any future tenant boundary. Migration:
   `migration/src/m20260826_000001_tenant.rs`.
2. **DDI tenant rejection:** raptor now answers to exactly one tenant (new
   `tenant` config key, default `DEFAULT`, matched case-insensitively —
   Zephyr's `CONFIG_HAWKBIT_TENANT` defaults to lowercase `"default"`) and
   404s any other segment, checked once in `ddi_auth` middleware ahead of
   every DDI route. This matches hawkBit's own behavior and closes the
   controllerId-collision hazard above. **This is a breaking change** for any
   deployment where a device was misconfigured with a stray tenant name — it
   worked (accidentally) before, and 404s now. The escape hatch is the new
   `tenant` config key.

No query anywhere filters on `tenant` yet, no DTO in `raptor-api-types`
carries it (hawkBit's don't either), and no auth path is tenant-aware beyond
the DDI segment check. Behavior is unchanged for every single-tenant
deployment using the default tenant name.

## Migration path for existing databases

The schema migration runs automatically at startup like every other raptor
migration. Every pre-existing row gets `tenant = 'DEFAULT'` via the column
default — no backfill step, no downtime beyond a normal migration run. An
operator who was relying on the old permissive DDI tenant-segment behavior
needs to either reconfigure their client to the default tenant name or set
the new `tenant` config key to match what their client already sends.
