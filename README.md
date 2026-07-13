# raptor

A [hawkBit](https://eclipse.dev/hawkbit/)-compatible OTA update server in Rust.
One binary, one config file. Speaks hawkBit's DDI v1 API (SWUpdate, RAUC
hawkbit-updater and other hawkBit clients work unchanged) and the core
Management API workflow.

## Quick start

    cargo build --release
    printf 'yourpassword\n' | ./target/release/raptor hash-password
    # put the hash in raptor.toml, then:
    ./target/release/raptor serve --config raptor.toml

Minimal `raptor.toml`:

    bind = "0.0.0.0:8080"
    database_url = "sqlite://raptor.db?mode=rwc"   # or postgres://user:pass@host/db
    artifact_dir = "./artifacts"

    [ddi]
    gateway_token = "change-me"        # or anonymous = true for dev

    [mgmt]
    username = "admin"
    password_hash = "$argon2id$..."

Deploy an update:

    # module + artifact + distribution set
    curl -u admin:pw -X POST localhost:8080/rest/v1/softwaremodules \
      -H 'Content-Type: application/json' \
      -d '[{"name":"rootfs","version":"1.0","type":"os"}]'
    curl -u admin:pw -X POST localhost:8080/rest/v1/softwaremodules/1/artifacts \
      -F 'file=@rootfs.img'
    curl -u admin:pw -X POST localhost:8080/rest/v1/distributionsets \
      -H 'Content-Type: application/json' \
      -d '[{"name":"release","version":"1.0","type":"os","modules":[{"id":1}]}]'
    # assign to a device (auto-registered on first poll)
    curl -u admin:pw -X POST localhost:8080/rest/v1/targets/my-device/assignedDS \
      -H 'Content-Type: application/json' -d '{"id":1,"type":"forced"}'

## Web UI

raptor ships an optional web console (Dioxus/WASM) embedded in the binary.

One-time setup, then a two-step build:

    rustup target add wasm32-unknown-unknown
    cargo binstall dioxus-cli@0.7.9  # or: cargo install dioxus-cli@0.7.9
    # pinned to match the crate's `dioxus = "=0.7.9"` — bump both together

    dx build --release --package raptor-ui    # from the repo root
    cargo build --release --features embed-ui

Then browse to `http://<server>/ui` and log in with the `[mgmt]` credentials.
The UI authenticates with an httpOnly session cookie (`POST /rest/v1/login`);
basic auth for curl/CI keeps working unchanged. CSRF note: the cookie is
`SameSite=Strict`, which blocks cross-site browser POSTs; there is no separate
CSRF token. Sessions live in memory — a server restart logs everyone out.

Without `--features embed-ui`, raptor builds and runs exactly as before and
`/ui` returns 404 — the Dioxus toolchain is only needed when embedding the UI.

Development loop (hot reload):

    cargo run -- serve --config raptor.toml     # terminal 1: API on :8080
    cd raptor-ui && dx serve                    # terminal 2: UI with /rest proxy

(`dx build --release` may print a non-fatal wasm-opt/DWARF warning — harmless,
the bundle still builds.)

Release smoke test: login → create module → upload artifact → create
distribution set → assign module → deploy to a target → watch the action on
the dashboard → cancel or complete → logout.

## v1 scope

DDI v1 + core Management API (targets, software modules, distribution sets,
artifacts, actions, rollouts, FIQL `q=` filtering) + embedded web console.
Rollouts cover the core lifecycle (create/start/pause/resume/delete, group
thresholds); approval workflow and dynamic rollouts are follow-ups. Not yet:
tags, target filters, AMQP/DMF. Design docs in `docs/superpowers/specs/`.
