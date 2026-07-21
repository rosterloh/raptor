# Configuration

raptor reads a single TOML file (default `raptor.toml`, override with
`--config`). Every key can also be set via a `RAPTOR_*` environment variable,
which is useful for secrets and containers.

## Minimal config

```toml
bind = "0.0.0.0:8080"
database_url = "sqlite://raptor.db?mode=rwc"   # or postgres://user:pass@host/db
artifact_dir = "./artifacts"

[ddi]
gateway_token = "change-me"        # or anonymous = true for dev

[mgmt]
username = "admin"
password_hash = "$argon2id$..."    # from `raptor hash-password`
```

Only `database_url`, `artifact_dir`, and the `[mgmt]` credentials are required;
everything else has a default. See the
[Configuration Reference](../reference/configuration.md) for every key and its
default.

## Environment overrides

Any key maps to an environment variable prefixed with `RAPTOR_`, with nested
tables joined by a double underscore (`__`):

```bash
export RAPTOR_BIND="127.0.0.1:9090"
export RAPTOR_DDI__ANONYMOUS=true
export RAPTOR_DDI__GATEWAY_TOKEN="super-secret"
export RAPTOR_MGMT__PASSWORD_HASH='$argon2id$...'
```

Environment values override the TOML file. This is the recommended way to inject
secrets — a plaintext gateway token in a world-readable config file is a
liability (the Debian package addresses this with a root-only `raptor.env`; see
[Running as a systemd Service](../guides/systemd.md)).

## Generating the admin password hash

The Management API and web console authenticate against a single admin
credential stored as an argon2id hash:

```console
$ printf 'yourpassword\n' | raptor hash-password
$argon2id$v=19$m=19456,t=2,p=1$...
```

Copy the output into `mgmt.password_hash`.

## Key groups at a glance

| Section | Purpose |
|---|---|
| top level | `bind`, `database_url`, `artifact_dir`, `max_artifact_size`, `url`, `rollout_eval_interval_secs` |
| `[ddi]` | device-facing auth and polling: `anonymous`, `gateway_token`, `polling_interval`, `confirmation_flow` |
| `[mgmt]` | Management API / web console admin: `username`, `password_hash` |

## The `url` key

By default the `_links` in API responses are derived from the incoming request's
`Host` header. If raptor sits behind a reverse proxy that rewrites the host, set
`url` to the externally visible base so devices receive dialable links:

```toml
url = "https://raptor.example.com"
```
