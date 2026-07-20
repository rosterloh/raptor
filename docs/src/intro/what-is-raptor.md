# What is raptor?

raptor is a Rust reimplementation of the [Eclipse hawkBit](https://eclipse.dev/hawkbit/)
server, targeting **drop-in API compatibility** with the parts of hawkBit that
device fleets use every day.

## The problem it solves

hawkBit is the de-facto open-source backend for embedded/IoT software updates.
Its device-facing **DDI** protocol is spoken by mature clients — SWUpdate's
suricatta module, the RAUC hawkbit-updater, and Collabora's `hawkbit` Rust crate.
But a production hawkBit deployment brings a heavy operational footprint: a JVM,
a servlet container, a relational database, and (for the DMF device path)
RabbitMQ.

raptor keeps the API and drops the weight:

| | hawkBit | raptor |
|---|---|---|
| Runtime | JVM / Spring Boot | single static binary |
| Database | MySQL / Postgres | SQLite **or** Postgres |
| Message broker | RabbitMQ (DMF) | none |
| Device API | DDI v1 + DMF | DDI v1 |
| Deployment | multi-service | one process, one config file |

Because raptor implements hawkBit's DDI v1 contract field-for-field, **existing
devices do not know the difference** — you point them at raptor's URL and they
poll, download, and report just as before.

## What raptor is

- A **device update server**: it tells devices what to install, serves the
  artifacts, and records their feedback.
- A **fleet management API**: create targets, upload firmware, compose
  distribution sets, assign updates, and run staged rollouts over REST.
- A **single-fleet, single-tenant** server. It accepts (and ignores) hawkBit's
  tenant URL segment, so clients configured for a tenant still work, but there
  is no tenant isolation. Run one raptor per fleet.

## What raptor is not (yet)

raptor implements a growing subset of hawkBit. It does **not** currently provide
the DMF (AMQP) device path, multi-tenancy, tags, target types, metadata
endpoints, maintenance windows, or the full set of action types. See
[hawkBit Compatibility](./compatibility.md) for the authoritative list.

## Design principles

- **Compatibility is the contract.** Clients branch on HTTP status codes and
  JSON field names; raptor matches hawkBit's exactly, backed by golden-fixture
  and end-to-end tests against a real hawkBit client.
- **One binary, one config file.** Everything — including the web console —
  ships in the executable. Configuration is a single TOML file with environment
  overrides.
- **Boring persistence.** One SeaORM entity/migration set targets both SQLite
  and Postgres; migrations run automatically at startup.
