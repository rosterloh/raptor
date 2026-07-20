# Configuration Reference

raptor reads a TOML file (default `raptor.toml`, override with
`serve --config <path>`). Every key can be overridden by a `RAPTOR_*`
environment variable; nested tables use a `__` separator (e.g.
`RAPTOR_DDI__ANONYMOUS`).

## Top level

| Key | Type | Default | Description |
|---|---|---|---|
| `bind` | socket addr | `0.0.0.0:8080` | address the HTTP server listens on |
| `database_url` | string | *(required)* | `sqlite://…` or `postgres://…`; selects the backend |
| `artifact_dir` | path | *(required)* | root of the content-addressed artifact store |
| `max_artifact_size` | integer (bytes) | `1073741824` (1 GiB) | maximum artifact upload size |
| `url` | string | *(unset)* | external base URL for `_links`; derived from the `Host` header when unset |
| `rollout_eval_interval_secs` | integer | `5` | how often the background evaluator / auto-assign sweep runs |

## `[ddi]` — device-facing API

| Key | Type | Default | Description |
|---|---|---|---|
| `anonymous` | bool | `false` | disable all DDI auth (dev only) |
| `gateway_token` | string | *(unset)* | shared token; enables auto-registration |
| `polling_interval` | string `HH:MM:SS` | `00:05:00` | poll sleep advertised to devices |
| `confirmation_flow` | bool | `false` | require confirmation before a deployment starts |

## `[mgmt]` — Management API / web console

| Key | Type | Default | Description |
|---|---|---|---|
| `username` | string | *(required)* | admin username |
| `password_hash` | string | *(required)* | argon2id hash from `raptor hash-password` |

## Example

```toml
bind = "0.0.0.0:8080"
database_url = "postgres://raptor:raptor@localhost/raptor"
artifact_dir = "/var/lib/raptor/artifacts"
max_artifact_size = 2147483648            # 2 GiB
url = "https://raptor.example.com"
rollout_eval_interval_secs = 10

[ddi]
anonymous = false
gateway_token = "shared-registration-secret"
polling_interval = "00:05:00"
confirmation_flow = true

[mgmt]
username = "admin"
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$..."
```

## Environment overrides

```bash
RAPTOR_BIND=127.0.0.1:9090
RAPTOR_DATABASE_URL=sqlite://raptor.db?mode=rwc
RAPTOR_DDI__ANONYMOUS=true
RAPTOR_DDI__GATEWAY_TOKEN=super-secret
RAPTOR_MGMT__PASSWORD_HASH='$argon2id$...'
```

Environment values take precedence over the TOML file — the recommended way to
inject secrets.
