# Architecture

raptor is a **modular monolith**: one process, one binary, with clear internal
seams between the HTTP layer, the domain logic, and persistence.

## Workspace

```
raptor/
├── Cargo.toml            # workspace
├── raptor/               # main crate (bin + lib)
│   └── src/
│       ├── main.rs           # CLI (serve, hash-password), startup, background tasks
│       ├── app.rs            # router assembly
│       ├── config.rs         # TOML + RAPTOR_* env overrides
│       ├── api/
│       │   ├── ddi/          # /{tenant}/controller/v1/... device handlers
│       │   └── mgmt/         # /rest/v1/... operator handlers
│       ├── domain/           # deployment state machine, rollouts, auto-assign
│       ├── entity/           # SeaORM entities
│       ├── fiql/             # FIQL/RSQL parser -> SeaORM Condition
│       ├── storage.rs        # content-addressed artifact store
│       └── auth/             # tower middleware for both auth zones
├── raptor-api-types/     # shared Management API DTOs (also compiles to wasm)
├── raptor-ui/            # Dioxus web console (wasm32)
└── migration/            # sea-orm-migration crate (one set, both DBs)
```

## Stack

- **[axum](https://github.com/tokio-rs/axum)** — HTTP routing and extractors.
- **[SeaORM](https://www.sea-ql.org/SeaORM/)** — one entity/migration definition
  targeting **both SQLite and Postgres**; the backend is chosen by the
  `database_url` scheme.
- **[tokio](https://tokio.rs/)** — async runtime; also drives background tasks.
- **[winnow](https://github.com/winnow-rs/winnow)** — the FIQL parser.
- **sha2 / sha1 / md-5** — artifact hashing.
- **argon2** — admin password hashing.
- **[Dioxus](https://dioxuslabs.com/)** — the optional embedded web console.

## Request path

1. A request hits axum. A **tower middleware** enforces the right auth zone —
   `ddi_auth` for `/{tenant}/controller/v1/...`, `mgmt_auth` for `/rest/v1/...`.
2. The **handler** (`api/ddi` or `api/mgmt`) validates input and calls into the
   **domain** layer.
3. The **domain** layer owns the rules — the action state machine, rollout
   evaluation, auto-assignment — and talks to persistence through **entities**.
4. **Artifact bytes** bypass the database: they stream to/from the
   content-addressed store on disk.

## Background tasks

A single tokio task runs on a fixed interval
(`rollout_eval_interval_secs`, default 5s) and performs two jobs:

- **Rollout evaluation** — advances or pauses running rollout groups based on
  action outcomes.
- **Auto-assignment sweep** — assigns target-filter distribution sets to
  newly-matching targets.

## Shared DTOs

The `raptor-api-types` crate holds the Management API request/response types. It
compiles to `wasm32` as well as native, so the server and the Dioxus web console
share exactly the same type definitions — the JSON contract can't drift between
them. Round-trip tests assert the JSON shape matches hawkBit.

## Persistence & migrations

There is **one** SeaORM entity set and **one** ordered list of migrations. They
run automatically at startup against whichever database `database_url` points to,
so upgrading raptor and migrating the schema is a single step: start the new
binary.
