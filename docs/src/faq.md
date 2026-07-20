# FAQ

## Is raptor a drop-in replacement for hawkBit?

For the **DDI v1 device protocol**, yes — devices using SWUpdate, the RAUC
hawkbit-updater, or other hawkBit DDI clients work unchanged. For the
**Management API**, raptor implements the core workflow but not every endpoint;
see [hawkBit Compatibility](./intro/compatibility.md).

## Which databases are supported?

SQLite and Postgres, selected by the `database_url` scheme. Migrations run
automatically at startup. SQLite is great for small/single-node deployments;
Postgres for larger fleets or when you want an external managed database.

## Does raptor need RabbitMQ?

No. raptor implements only the HTTP-based DDI device path, not hawkBit's DMF
(AMQP) path, so there is no broker to run.

## Can I run multiple tenants on one raptor?

No. raptor is single-tenant. It accepts and ignores the tenant segment in DDI
URLs (so tenant-configured clients still work), but there is no data isolation.
Run one raptor instance per fleet.

## How do devices authenticate?

Per-target security tokens, a shared gateway token (which also enables
auto-registration), or anonymous mode for development. See
[Authentication](./guides/authentication.md).

## Where does raptor store artifacts?

In a content-addressed store on local disk under `artifact_dir`, laid out as
`<sha256[0..2]>/<sha256>`. Identical content is stored once. The Debian package
defaults this to `/var/lib/raptor/artifacts`.

## Can I offload artifact storage to S3?

Not yet — storage is local disk only. S3/object-storage backends are a tracked
enhancement.

## How do I change the admin password?

Generate a new hash with `raptor hash-password`, put it in `mgmt.password_hash`
(or the `RAPTOR_MGMT__PASSWORD_HASH` env var), and restart. There is a single
admin user in this version.

## How do I make a deployment wait for approval?

Enable the [confirmation flow](./guides/confirmation-flow.md)
(`[ddi] confirmation_flow = true`). Assignments then wait for the device (or an
operator via auto-confirm) before deploying. Note this is per-assignment
confirmation, not hawkBit's separate rollout *approval* workflow, which isn't
implemented.

## Why is my `deploymentBase` returning 404?

The action is not in a state that serves a deployment. If the confirmation flow
is on, the device should be using `confirmationBase` until the action is
confirmed; `deploymentBase` only serves `running` actions. Check the action's
`detailStatus` via `GET /rest/v1/targets/{cid}/actions/{aid}`.

## How do I upgrade raptor?

Install the new binary/package and restart. Schema migrations apply automatically
on startup. With the Debian package, your `/etc/raptor/config.toml` edits are
preserved (it's a dpkg conffile).

## Where do I report bugs or request features?

On the [GitHub repository](https://github.com/rosterloh/raptor/issues). The
[compatibility matrix](./intro/compatibility.md) lists what's not yet
implemented; most of those items have tracking issues.
