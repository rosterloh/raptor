# tenancy Specification

## Purpose

raptor answers to a single hawkBit tenant per instance. The schema is prepared
for real isolation later without a migration, but nothing is isolated today.

## Requirements

### Requirement: One tenant per instance
raptor SHALL serve exactly one tenant, named by the `tenant` config key (default `DEFAULT`). DDI requests for any other tenant SHALL get `404` (see `ddi-auth`). Operators needing several fleets SHALL run one instance per fleet.

#### Scenario: Custom tenant name
- **WHEN** `tenant = "acme"` and a device polls `/ACME/controller/v1/dev-1`
- **THEN** the tenant check passes

### Requirement: Management API has no tenant dimension
The Management API SHALL carry no tenant in its paths or DTOs, matching hawkBit, where the tenant comes from the authenticated principal.

#### Scenario: Listing targets
- **WHEN** an operator lists `/rest/v1/targets`
- **THEN** every target in the database is in scope

### Requirement: Query-root tables carry a tenant column
Every query-root table SHALL have a `tenant` column defaulting to `DEFAULT`. Unique names SHALL be named composite indexes that include `tenant`, never inline unique constraints, so they can be widened on SQLite without a table rebuild. New query-root tables SHALL follow the same rule.

#### Scenario: New uniquely-named table
- **WHEN** a migration adds a table with a unique `name`
- **THEN** it has a `tenant` column and a named unique index on `(tenant, name)`

### Requirement: Artifacts are tenant-neutral
Artifact blobs SHALL be addressed by content hash alone, with no tenant, so deduplication and reference counting stay correct across any future tenant boundary.

#### Scenario: Same bytes, two modules
- **WHEN** two modules upload identical bytes
- **THEN** one blob is stored, whatever tenant either belongs to
