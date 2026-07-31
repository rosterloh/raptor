# raptor — agent/contributor guide

A hawkBit-compatible OTA update server in Rust: one binary, one config file.
Devices speak the DDI v1 API; operators use the hawkBit Management API and an
optional embedded web console. Full docs live in `docs/` (mdbook) — start with
`docs/src/concepts/architecture.md` for the layering and request path.

## Workspace map

- `raptor/` — main crate (bin + lib)
  - `src/api/ddi/` — device-facing handlers (`/{tenant}/controller/v1/...`)
  - `src/api/mgmt/` — operator-facing handlers (`/rest/v1/...`); each module
    owns its own routes; `mappers.rs` maps entities → REST DTOs
  - `src/domain/` — business rules: deployment/action state machine, rollout
    evaluation, target-filter auto-assignment (handlers stay thin)
  - `src/entity/` — SeaORM entities (one definition for SQLite and Postgres)
  - `src/fiql/` — FIQL/RSQL `q=` filter parser → SeaORM `Condition`
  - `src/auth/` — tower middleware for the two auth zones (ddi, mgmt)
  - `src/storage.rs` — content-addressed artifact store on disk
  - `tests/` — integration tests, one file per feature (`mgmt_*`, `ddi_*`),
    shared harness in `tests/common/`
- `raptor-api-types/` — shared Management API DTOs, one module per resource.
  Must stay wasm32-compatible: serde/serde_json only, no new dependencies.
- `raptor-ui/` — Dioxus/WASM web console; HTTP client in `src/api/` (one
  module per resource), pages in `src/pages/`
- `migration/` — sea-orm-migration crate; files named `mYYYYMMDD_NNNNNN_slug`
- `docs/` — mdbook (`mdbook build docs`). NOTE: `docs/superpowers/` holds
  *historical* design docs and plans — the code is the source of truth.

## Commands

```sh
cargo nextest run --workspace          # tests (or: cargo test --workspace)
cargo test -p raptor --features otel --test telemetry   # otel-gated test
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p raptor --features otel --all-targets -- -D warnings
cargo clippy -p raptor-ui --target wasm32-unknown-unknown -- -D warnings
```

Web console (only needed for `embed-ui` work; pinned `dioxus-cli` version must
match the crate's `dioxus = "=0.7.10"` — bump both together):

```sh
cargo binstall dioxus-cli@0.7.10
dx build --release --package raptor-ui   # from the repo root, THEN:
cargo build --release --features embed-ui
```

`dx build` rescans the sources and rewrites the committed
`raptor-ui/assets/tailwind.css`. Any change that adds or drops a Tailwind
utility class must commit the regenerated stylesheet — CI diffs it, because a
missing class compiles, lints, and tests clean while silently rendering wrong.

Feature flags on `raptor`: `embed-ui` (serve the console at `/ui`), `otel`
(OTLP traces/metrics/logs). Both off by default; CI lints/tests both.

## Conventions

- **Adding a Management API endpoint**: DTO in `raptor-api-types` (with a
  round-trip JSON test in its `tests.rs`) → handler + route in the matching
  `raptor/src/api/mgmt/` module → entity→REST mapping in `mappers.rs` →
  integration test in `raptor/tests/mgmt_<feature>.rs` → document it in
  `docs/src/reference/management-api.md` and the relevant guide.
- **hawkBit compatibility is the contract**: JSON field names, paging
  envelope, and error bodies must match hawkBit; don't "improve" the wire
  format. DDI behavior must keep stock clients (SWUpdate, rauc-hawkbit-updater)
  working unchanged.
- **Schema changes**: new migration file in `migration/src/` (never edit an
  existing migration) + matching entity change; must work on both SQLite and
  Postgres. Migrations run automatically at startup.
- **Layering**: handlers validate and translate; rules live in `src/domain/`;
  artifact bytes stream through `storage.rs`, never through the DB.
- Keep `cargo fmt` and `clippy -D warnings` clean — CI enforces both.
