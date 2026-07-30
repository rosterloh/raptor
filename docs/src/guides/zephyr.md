# Using raptor with Zephyr

Zephyr's mainline hawkBit client (`subsys/mgmt/hawkbit`) works against raptor's
DDI API unchanged. This page is the minimal wiring, plus the two footguns worth
knowing before you flash a fleet.

Kconfig symbols below are from Zephyr's `subsys/mgmt/hawkbit/Kconfig`; the
compatibility matrix at the end records what the client actually exercises.

## Minimal configuration

Device side (`prj.conf`):

```kconfig
CONFIG_HAWKBIT=y
CONFIG_HAWKBIT_SERVER="ota.example.com"
CONFIG_HAWKBIT_PORT=8088
CONFIG_HAWKBIT_TENANT="DEFAULT"
CONFIG_HAWKBIT_POLL_INTERVAL=5          # minutes; 1..43200

# Pick one auth mode (see below)
CONFIG_HAWKBIT_DDI_GATEWAY_SECURITY=y
CONFIG_HAWKBIT_DDI_SECURITY_TOKEN="shared-registration-secret"

# If raptor is behind TLS
# CONFIG_HAWKBIT_USE_TLS=y
```

If `CONFIG_HAWKBIT_SERVER` is a hostname rather than an IP, the device also needs
`CONFIG_DNS_RESOLVER=y`.

Server side (`raptor.toml`):

```toml
url = "http://ota.example.com:8088"

[ddi]
gateway_token = "shared-registration-secret"
polling_interval = "00:05:00"
```

Keep `polling_interval` and `CONFIG_HAWKBIT_POLL_INTERVAL` consistent — raptor
advertises its value as `config.polling.sleep` (`HH:MM:SS`), which the client
reads and honours, so the Kconfig value is really just the pre-first-poll default.

## Choosing an auth mode

The client sends exactly one of two headers, selected at build time:

| Kconfig choice | Header sent | raptor side |
|---|---|---|
| `CONFIG_HAWKBIT_DDI_GATEWAY_SECURITY=y` | `Authorization: GatewayToken <token>` | `[ddi] gateway_token = "<token>"` |
| `CONFIG_HAWKBIT_DDI_TARGET_SECURITY=y` | `Authorization: TargetToken <token>` | the target's own `securityToken` |

Either way the token value goes in `CONFIG_HAWKBIT_DDI_SECURITY_TOKEN`.

**Gateway token** is one shared secret for the whole fleet and permits
auto-registration: a device that polls with it is created on first contact. Good
for bringing a fleet up, weaker blast radius if extracted from one device.

**Target token** is per-device, so the target must exist in raptor first, with a
token you provision into the firmware:

```bash
curl -u admin:pw -X POST localhost:8088/rest/v1/targets \
  -H 'Content-Type: application/json' \
  -d '[{"controllerId": "device-42", "securityToken": "per-device-secret"}]'
```

See [Authentication](./authentication.md) for the full picture. `[ddi] anonymous
= true` disables DDI auth entirely — convenient for a bring-up on a lab network,
never in production.

## Footgun 1: the tenant must be `DEFAULT`

Zephyr's `CONFIG_HAWKBIT_TENANT` defaults to `"default"`, and raptor is
single-tenant: it accepts any tenant segment but emits `DEFAULT` in every link it
returns. Case doesn't matter (`default` and `DEFAULT` are the same tenant here),
but anything else leaves the device following hrefs that disagree with its own
configuration, and would break outright if multi-tenancy ever lands. raptor logs a
warning the first time it sees one. Set it to `DEFAULT` and move on.

## Footgun 2: don't enable the confirmation flow

The client parses exactly three `_links` keys from the base poll —
`deploymentBase`, `configData`, `cancelAction`. It has **no `confirmationBase`
handling at all**.

So with `[ddi] confirmation_flow = true`, a waiting target is offered only
`confirmationBase`, the device sees no link it understands, and it polls forever
without installing and without reporting an error. If you need the flow for other
clients, keep Zephyr devices out of it with:

```toml
[ddi]
confirmation_flow = true
auto_confirm_default = true    # new targets auto-confirm
```

See [Confirmation Flow](./confirmation-flow.md).

## Compatibility matrix

What the mainline Zephyr client uses, and raptor's status for each. "Verified"
means covered by raptor's integration tests, including a JSON-contract test
(`zephyr_client_json_contract`) pinning the exact response shape the client's
strict JSON descriptors require.

| DDI feature | Zephyr client | raptor |
|---|---|---|
| Base poll `config.polling.sleep` (`HH:MM:SS`) | reads and honours | ✅ verified |
| `_links.deploymentBase` | parsed | ✅ verified |
| `_links.configData` | parsed; uploads whenever present | ✅ verified — advertised only when attributes are wanted, so devices don't re-upload every poll |
| `_links.cancelAction` | parsed | ✅ verified |
| `_links.confirmationBase` | **not parsed** | ⚠️ raptor supports it; do not enable for Zephyr (footgun 2) |
| `configData` PUT, `mode: "merge"` | sends on every poll carrying the link | ✅ verified |
| `deploymentBase` chunks/artifacts | parsed | ✅ verified |
| Artifact `hashes.sha256` | verified against flashed image | ✅ verified |
| Artifact `_links.download-http` | the link it downloads from | ✅ verified — see [`download-http` vs `download`](../reference/ddi-api.md#download-http-vs-download) |
| `Range:` resume (`CONFIG_HAWKBIT_SAVE_PROGRESS`) | sends `bytes=N-` | ✅ verified — `206` + `Content-Range` |
| Feedback `execution` / `result` | `closed`, `proceeding`, `canceled`, `scheduled`, `rejected`, `resumed`, `none` / `success`, `failure`, `none` | ✅ verified |
| `Authorization: TargetToken` / `GatewayToken` | either, chosen at build time | ✅ verified |
| Multi-tenancy | `CONFIG_HAWKBIT_TENANT` | ⚠️ single-tenant, `DEFAULT` only (footgun 1) |

## Verifying against a real device

raptor's test suite drives its DDI API with the Rust `hawkbit` crate and the
JSON-contract test above — not with the Zephyr client itself, which needs
hardware or QEMU. For an end-to-end check, build Zephyr's
`samples/subsys/mgmt/hawkbit` sample against a raptor instance and watch the
server log: a successful cycle is a base poll, a `configData` PUT, a
`deploymentBase` GET, artifact GETs, then feedback with `execution: "closed"`.
