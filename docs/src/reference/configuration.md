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
| `artifact_dir` | path | *(required)* | root of the content-addressed artifact store; with `[storage] backend = "s3"` it only holds uploads in flight |
| `max_artifact_size` | integer (bytes) | `1073741824` (1 GiB) | maximum artifact upload size |
| `url` | string | *(unset)* | external base URL for `_links`; derived from the `Host` header when unset |
| `rollout_eval_interval_secs` | integer | `5` | how often the background evaluator / auto-assign sweep runs |
| `rollout_approval_enabled` | bool | `false` | gate new rollouts behind an operator approval (`waiting_for_approval`); reported as hawkBit's `rollout.approval.enabled` |
| `tenant` | string | `DEFAULT` | tenant name this instance answers to on the DDI `/{tenant}/...` path segment (matched case-insensitively); any other segment gets `404` |

## `[quota]` — per-entity growth caps

Mirrors hawkBit's quotas (`hawkbit.server.security.dos.*`), with its default
values. These bound unbounded growth — a chatty device appending action-status
rows forever, a runaway upload loop — rather than rate-limiting requests. A
breach is rejected with `429 Too Many Requests` and hawkBit's
`hawkbit.server.error.quota.tooManyEntries` error code.

**Set a key to `0` to disable that quota.** This matches hawkBit, which treats
any limit `<= 0` as unlimited.

| Key | Default | Caps |
|---|---|---|
| `max_status_entries_per_action` | `1000` | status entries a device may report against one action |
| `max_messages_per_action_status` | `50` | messages a device may attach to one reported status |
| `max_attribute_entries_per_target` | `100` | attributes a device may report about itself |
| `max_metadata_entries_per_target` | `100` | metadata entries per target |
| `max_metadata_entries_per_software_module` | `100` | metadata entries per software module |
| `max_metadata_entries_per_distribution_set` | `100` | metadata entries per distribution set |
| `max_artifacts_per_software_module` | `50` | artifacts per software module |
| `max_software_modules_per_distribution_set` | `100` | modules per distribution set |
| `max_rollout_groups_per_rollout` | `500` | deployment groups per rollout |
| `max_targets_per_rollout_group` | `20000` | targets in any one rollout group |

The two device-reported caps apply only to what a *device* sends over DDI.
raptor's own status entries — "rollout stopped", "superseded by a new
assignment" — are never capped, so a device that has exhausted its quota cannot
stop the server recording why its action was cancelled.

`max_status_entries_per_action` also exempts feedback that *closes* an action
(`closed`, `canceled`, a `downloadonly` action's `downloaded`, and a cancel's
`closed`/`rejected`). Without that carve-out a device that spent its budget on
progress reports could never file its terminal one, leaving the action active
forever with no way to close it. hawkBit does the same, checking the count only
for intermediate statuses.

Artifact *size* is capped separately by the top-level `max_artifact_size`.

## `[cleanup]` — automatic action cleanup

Deletes closed actions past a retention window, along with their status
history. `action_status` is the one table that otherwise grows without bound:
quotas cap how much history any *single* action can accumulate, but nothing
caps how many actions a fleet accumulates over years of updates.

Off by default — deleting deployment history is not something to start doing to
an existing installation unasked.

| Key | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `false` | run the sweep at all |
| `action_expiry_days` | integer | `30` | how long a closed action is kept, from when it was last modified |
| `action_statuses` | list | `["finished", "error", "canceled"]` | which statuses may be deleted |
| `interval_secs` | integer | `3600` | how often the sweep runs |

```toml
[cleanup]
enabled = true
action_expiry_days = 90
```

An **active action is never eligible**, whatever `action_statuses` says — a
device can hold a listed status while its action is still live, and deleting it
would strand that device holding an action id the server no longer knows. Deletion is
batched — 1000 actions per statement, up to 10k per sweep — so a long-neglected
instance drains over successive sweeps rather than in one enormous statement.
(hawkBit bounds its own cleanup the same way, but at one batch per run.)

Rollout progress is unaffected. `totalTargetsPerStatus` is derived by counting
actions, so deleting a finished one would walk a completed rollout's targets
back to `scheduled` — reporting as though the deployment had never run. raptor
records each group's purged outcomes before the rows go and folds them back in.
(hawkBit has this drift and tolerates it; raptor does not.)

The two hawkBit tenant config keys are reported on
`/rest/v1/system/configs`: `action.cleanup.auto.expiry` (milliseconds, `-1`
when disabled) and `action.cleanup.auto.status`.

## `[retention]` — release retention

Keeps the newest `keep_releases` releases of each distribution-set **name** and
hard-deletes everything older, freeing the artifact storage it held. A raptor
extension: hawkBit has no equivalent, so nothing here is reported on
`/rest/v1/system/configs`.

Off by default, because it deletes releases and their artifacts outright.
Setting `keep_releases` is what turns it on. There is no `enabled` flag:
leave `keep_releases` unset and every release is kept.

| Key | Type | Default | Description |
|---|---|---|---|
| `keep_releases` | integer | *(unset)* | valid sets kept per name, newest by creation time; at least 1. Unset keeps everything |
| `interval_secs` | integer | `3600` | how often the sweep runs |

```toml
[retention]
keep_releases = 5   # keep the 5 newest releases of each name, delete older ones
```

Releases are ranked by **creation time**, not by version string, because
version strings aren't reliably comparable. Sets that were never deployed still
count towards `keep_releases`. Invalidated sets don't, so an invalidated release
never takes up a rollback slot. Every set older than the oldest kept one is
retired, invalid or not. An invalidated set newer than that stays until enough
valid releases have followed it. A name with fewer than `keep_releases` valid
sets loses nothing.

Retiring a set:

1. **Invalidates** it with `force` and cancels its rollouts, exactly as
   `POST /distributionsets/{id}/invalidate` would.
2. **Deletes** its rollouts, its actions and their status history, and its
   metadata and tag assignments, then the set itself.
3. **Clears target references.** A target that had it installed shows nothing
   installed and goes back to `registered`. A pending assignment of it falls
   back to whatever the target has installed.
4. **Deletes every software module** that no remaining set uses, along with
   its artifacts. The blob goes from storage only when no other artifact
   shares its hash, so a module or file shared with a kept release survives.

Nothing is held back for a device still running a retired release. A device
that hasn't taken any of the last `keep_releases` releases needs attention
anyway, and assigning it a current release fixes it. Up to 50 sets are retired
per sweep, oldest first, so a first sweep over a long history drains over a few
intervals.

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

## `[storage]` — artifact backend

Where artifact blobs live. Omit the section to keep them on local disk under
`artifact_dir` (the default and the behaviour of every earlier release).

| Key | Type | Default | Description |
|---|---|---|---|
| `backend` | `"fs"` \| `"s3"` | `"fs"` | local disk, or an S3-compatible bucket configured in `[storage.s3]` |

### `[storage.s3]`

Any S3-compatible store: AWS S3, or a self-hosted one such as RustFS, MinIO,
Garage, Ceph RGW or TrueNAS. The defaults are chosen for self-hosted stores.

| Key | Type | Default | Description |
|---|---|---|---|
| `endpoint` | URL | *(required)* | S3 API base URL, e.g. `https://s3.lan:9000` |
| `bucket` | string | *(required)* | bucket name; must already exist |
| `access_key` | string | *(required)* | access key ID |
| `secret_key` | string | *(required)* | secret key — prefer `RAPTOR_STORAGE__S3__SECRET_KEY` |
| `region` | string | `"us-east-1"` | region used for request signing; self-hosted stores accept any value |
| `prefix` | string | `""` | key prefix inside the bucket, e.g. `"raptor/"` |
| `path_style` | bool | `true` | `endpoint/bucket/key` addressing; set `false` for AWS virtual-hosted buckets |
| `ca_cert` | path | *(none)* | PEM file of extra CA certificate(s) to trust, for a private CA or self-signed endpoint; added to the system roots |
| `part_size` | integer (bytes) | `16777216` (16 MiB) | multipart part size (minimum 5 MiB); smaller blobs are a single PUT |

Blobs are keyed `<prefix><sha256[0..2]>/<sha256>`, the same layout as on disk.
raptor still serves every download itself (devices never talk to the bucket),
so DDI authentication, `Range` requests and download progress behave exactly as
with local storage. The bucket is checked with a `HEAD` request at startup, so
bad credentials or a missing bucket stop `raptor serve` immediately.

Uploads are spooled to `<artifact_dir>/.tmp` while they are hashed, then sent to
the bucket, so `artifact_dir` needs room for the largest upload in flight.

To move an existing deployment from `fs` to `s3`, run
[`raptor storage-backfill`](./cli.md#raptor-storage-backfill) and then switch
`backend`.

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
rollout_approval_enabled = false

[quota]
max_artifacts_per_software_module = 20
max_status_entries_per_action = 0          # unlimited

[storage]
backend = "s3"

[storage.s3]
endpoint = "https://rustfs.lan:9000"
bucket = "raptor"
access_key = "raptor"
# secret_key from RAPTOR_STORAGE__S3__SECRET_KEY
ca_cert = "/etc/raptor/lan-ca.pem"

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
RAPTOR_STORAGE__S3__ACCESS_KEY=raptor
RAPTOR_STORAGE__S3__SECRET_KEY=s3-secret
```

Environment values take precedence over the TOML file — the recommended way to
inject secrets.
