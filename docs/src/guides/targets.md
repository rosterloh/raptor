# Targets & Auto-Registration

A **target** is a device raptor can update, identified by a unique
`controllerId`. Targets carry a security token, a reported set of attributes, and
an `updateStatus`.

## Update status

Every target has an `updateStatus` reflecting where it is in the update cycle:

| Status | Meaning |
|---|---|
| `unknown` | created via the Management API, never polled |
| `registered` | known to the server, no update assigned |
| `pending` | an update is assigned and in progress |
| `in_sync` | running the assigned distribution set |
| `error` | the last deployment failed |

## Creating targets

### Explicitly (Management API)

```bash
curl -u admin:pw -X POST localhost:8088/rest/v1/targets \
  -H 'Content-Type: application/json' \
  -d '[{"controllerId":"device-42","name":"Device 42"}]'
```

The request body is an array, so you can create many at once. A `securityToken`
is generated if you don't supply one.

### Automatically (auto-registration)

An unknown `controllerId` that polls the DDI API is created on the spot with
status `registered` — hawkBit's plug-and-play behavior. Auto-registration
requires the poll to be authenticated by the shared gateway token, or DDI
anonymous mode to be on. See [Authentication](./authentication.md).

## Listing and filtering

The list endpoint supports paging, sorting, and FIQL:

```bash
curl -u admin:pw 'localhost:8088/rest/v1/targets?offset=0&limit=50&sort=controllerId:ASC'
curl -u admin:pw 'localhost:8088/rest/v1/targets?q=updateStatus==error'
```

Filterable fields include `controllerId` (alias `id`), `name`, `description`,
`updateStatus`, `lastControllerRequestAt`, `address`, and `group`. See
[Filtering with FIQL](./fiql.md).

## Groups

A target can carry one **group**: its organisational placement in the fleet —
which plant, which customer, which vehicle line. It is a plain string, and a
`/` in it is just a character, so nesting is a naming convention rather than a
structure the server enforces.

```bash
# set one at registration, or move a device later
curl -u admin:pw -X POST localhost:8088/rest/v1/targets \
  -H 'Content-Type: application/json' \
  -d '[{"controllerId": "dev-1", "group": "plant-a/line-3"}]'

curl -u admin:pw -X PUT localhost:8088/rest/v1/targets/dev-1 \
  -H 'Content-Type: application/json' -d '{"group": "plant-b/line-1"}'
```

Because `==` supports `*` wildcards, the naming convention is what makes a
hierarchy queryable — `plant-a/*` matches every line in plant A:

```bash
curl -u admin:pw 'localhost:8088/rest/v1/targets?q=group==plant-a/*'
curl -u admin:pw 'localhost:8088/rest/v1/targets?q=group==plant-a/*;updateStatus==error'
```

The same query works in a saved [target filter](./target-filters.md) and in a
rollout, since all three share one FIQL compiler.

### Groups vs. tags vs. types

The three look similar and answer different questions:

| | How many per target | Constrains anything | Answers |
|---|---|---|---|
| **Group** | at most one | no | *where does this device sit in the fleet* |
| **[Tags](./tags.md)** | many | no | *what is true about this device right now* |
| **[Target type](./distribution-sets.md)** | at most one | yes — which DS types may be assigned | *what kind of device is it* |

A device belongs to one plant but can be both `beta` and `field-trial`; the type
is the only one of the three that can refuse an assignment.

Omitting `group` from a `PUT` leaves the current one unchanged. As with
`description`, there is no way to clear it back to unset through the update
body.

## Last-seen address

A target's `address` / `ipAddress` is recorded from its DDI polls, so a device
that self-registers shows one without any Management API call. By default it's
the socket peer address.

Behind a reverse proxy the socket peer is the proxy, so point raptor at the
header carrying the real address:

```toml
[ddi]
trusted_proxy_header = "x-forwarded-for"
```

This is unset by default because a device can put whatever it likes in that
header — only set it when a proxy you control is rewriting it. raptor reads the
**rightmost** entry, the hop appended by the proxy directly in front of it;
entries to the left are caller-supplied and so spoofable. Values that don't parse
as an IP are ignored. Both `X-Forwarded-For` lists and RFC 7239 `Forwarded`
(`for=…`) syntax are understood, with or without a port.

## Attributes

Devices report key/value **attributes** (hardware revision, OS version, …) via
the DDI `configData` endpoint. Retrieve them with:

```bash
curl -u admin:pw localhost:8088/rest/v1/targets/device-42/attributes
# {"hw":"rev2","os":"linux"}
```

Attributes are set by the device, in three modes — `merge` (default), `replace`,
and `remove` — described in the [DDI API reference](../reference/ddi-api.md).

> **Note:** target *attributes* (device-reported) are distinct from hawkBit
> *metadata* (operator-set key/value pairs), which raptor does not yet implement.

## Poll status

When a target has polled at least once, its representation includes a
`pollStatus` block with the last request time, the next expected request time
(derived from the configured polling interval), and an `overdue` flag — handy for
spotting devices that have gone quiet.
