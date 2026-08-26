# Configuration Reference

raptor reads a TOML file (default `raptor.toml`, override with
`serve --config <path>`). Every key can be overridden by a `RAPTOR_*`
environment variable; nested tables use a `__` separator (e.g.
`RAPTOR_DDI__ANONYMOUS`).

## Top level

| Key | Type | Default | Description |
|---|---|---|---|
| `bind` | socket addr | `0.0.0.0:8088` | address the HTTP server listens on |
| `database_url` | string | *(required)* | `sqlite://…` or `postgres://…`; selects the backend |
| `artifact_dir` | path | *(required)* | root of the content-addressed artifact store |
| `max_artifact_size` | integer (bytes) | `1073741824` (1 GiB) | maximum artifact upload size |
| `url` | string | *(unset)* | external base URL for `_links`; derived from the `Host` header when unset |
| `rollout_eval_interval_secs` | integer | `5` | how often the background evaluator / auto-assign sweep runs |
| `tenant` | string | `DEFAULT` | tenant name this instance answers to on the DDI `/{tenant}/...` path segment (matched case-insensitively); any other segment gets `404` |

## `[ddi]` — device-facing API

| Key | Type | Default | Description |
|---|---|---|---|
| `anonymous` | bool | `false` | disable all DDI auth (dev only) |
| `gateway_token` | string | *(unset)* | shared token; enables auto-registration |
| `polling_interval` | string | `00:05:00` | poll sleep advertised to devices — see [Polling time overrides](#polling-time-overrides) below |
| `confirmation_flow` | bool | `false` | require confirmation before a deployment starts |
| `auto_confirm_default` | bool | `false` | give newly created targets `autoConfirm`, so `confirmation_flow` can't strand confirmation-unaware clients |
| `artifact_http_url` | string | *(unset)* | plain-HTTP base advertised in the DDI `download-http` links; unset means they reuse `url` |
| `trusted_proxy_header` | string | *(unset)* | header to read the device address from behind a reverse proxy, e.g. `x-forwarded-for`; unset uses the socket peer |

### Polling time overrides

`polling_interval` accepts hawkBit's `pollingTime` grammar (hawkBit 0.10,
[PR #2533](https://github.com/eclipse-hawkbit/hawkbit/pull/2533)): a default
interval, optionally followed by ordered `<RSQL> -> <interval>` override
rules, evaluated in written order — first match wins:

```
<default>[, <RSQL> -> <interval>]*
```

Each interval is `HH:MM:SS`, optionally with `~NN%` jitter (`NN` 0–99):
raptor draws fresh randomness on every poll response, matching hawkBit
exactly, so a device's advertised sleep can vary poll to poll rather than
settling on one value.

```toml
[ddi]
polling_interval = "00:05:00, group==eu -> 00:00:30~10%, updateStatus!=in_sync -> 00:02:00"
```

- The bare `HH:MM:SS` form (no rules) is unchanged from before this feature
  and behaves identically.
- Override filters use **raptor's own FIQL dialect** — the same one every
  `q=` parameter accepts — not hawkBit's fuller Spring RSQL grammar. In
  particular, whitespace around the operator is **not** tolerated: write
  `group==eu`, not hawkBit's own doc example `group == 'eu'`. A rule that
  fails to parse is rejected at startup (see below), not silently ignored.
- raptor does **not** support hawkBit's multi-day (`d+:HH:mm:ss`) or
  ISO-8601 (`P2DT3H4M`) interval forms — the incident-shaped use case this
  exists for ("poll this device faster while I'm watching it") doesn't need
  day-scale intervals.
- A malformed `polling_interval` — bad grammar, or a rule referencing a
  filter field raptor doesn't recognize — fails `raptor serve` at startup
  with an error, rather than surfacing on a device's poll.
- **Known limitation, inherited from hawkBit:** the Management API's
  `pollStatus.overdue` is always computed from the *default* interval, even
  for a target currently matched by an override rule. hawkBit's own release
  notes call this out as an accepted inaccuracy rather than a bug to fix.

## `[mgmt]` — Management API / web console

| Key | Type | Default | Description |
|---|---|---|---|
| `username` | string | *(required)* | admin username |
| `password_hash` | string | *(required)* | argon2id hash from `raptor hash-password` |

## Example

```toml
bind = "0.0.0.0:8088"
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
