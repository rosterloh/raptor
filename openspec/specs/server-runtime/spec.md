# server-runtime Specification

## Purpose

How the raptor binary starts and is configured: one TOML file with environment
overrides, SQLite or Postgres, automatic migrations, and the base URL used in
links.

## Requirements

### Requirement: Single config file with environment overrides
`raptor serve` SHALL read one TOML file (default `raptor.toml`, or `--config`). Every key SHALL be overridable by a `RAPTOR_*` variable, with `__` separating nested tables, and environment values SHALL take precedence.

#### Scenario: Secret from environment
- **WHEN** `RAPTOR_MGMT__PASSWORD_HASH` is set
- **THEN** it overrides `[mgmt] password_hash` from the file

### Requirement: Backward-compatible config
New features SHALL be opt-in. A config valid for an earlier release SHALL keep working with unchanged behaviour.

#### Scenario: Upgrade
- **WHEN** an operator upgrades raptor without editing `raptor.toml`
- **THEN** the server starts and behaves as before

### Requirement: Database backends and migrations
`database_url` SHALL select SQLite or Postgres. Pending migrations SHALL be applied at startup, before listening, and every migration SHALL work on both backends.

#### Scenario: Fresh Postgres
- **WHEN** raptor starts against an empty Postgres database
- **THEN** the schema is created before the first request is served

### Requirement: External base URL
`_links` SHALL use `url` when set, and otherwise be derived from the request's `Host` header.

#### Scenario: Behind a proxy
- **WHEN** `url = "https://ota.example.com"`
- **THEN** DDI links start with `https://ota.example.com`

### Requirement: Health probe
`GET /health` SHALL return `ok` without authentication.

#### Scenario: Liveness
- **WHEN** an orchestrator probes `/health`
- **THEN** the response is `200 ok`
