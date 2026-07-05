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

## v1 scope

DDI v1 + core Management API (targets, software modules, distribution sets,
artifacts, actions, FIQL `q=` filtering). Not yet: rollouts, tags, target
filters, UI, AMQP/DMF. Design docs in `docs/superpowers/specs/`.
