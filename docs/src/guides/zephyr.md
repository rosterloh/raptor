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

## Footgun 1: the tenant must match

Zephyr's `CONFIG_HAWKBIT_TENANT` defaults to `"default"`. raptor answers to
exactly one tenant, set by the `tenant` config key (default `DEFAULT`, matched
case-insensitively, so the Zephyr default just works out of the box) — any
other segment gets `404` on every DDI request. If your fleet is genuinely
configured with a non-default tenant name, set `tenant` in `raptor.toml` to
match rather than reconfiguring every device.

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
| Multi-tenancy | `CONFIG_HAWKBIT_TENANT` | ⚠️ single-tenant, must match configured `tenant` (footgun 1) |

## Sharing a server with Linux devices

Zephyr devices and Linux devices running SWUpdate can poll the same raptor. One
client limit makes the separation matter: Zephyr's client rejects any
deployment that isn't **exactly one chunk with exactly one artifact**
(`hawkbit.c`, "Expecting 1 chunk" / "Expecting 1 artifact"). A Linux set with
two modules, or a [zchunk delta](./swupdate.md#delta-updates-zchunk) module
holding both a `.swu` and a `.zck`, is refused by a Zephyr device before it
downloads anything.

raptor needs no special handling for this. Target types already keep the two
apart: a typed target accepts only distribution sets of a compatible type, and
direct assignment, auto-assigning target filters and rollouts all enforce that.
Give MCU sets their own type built on the seeded `firmware` module type, and
type every device:

```bash
# A distribution-set type that needs one firmware module (look up its id first)
curl -u admin:pw localhost:8088/rest/v1/softwaremoduletypes
curl -u admin:pw -H 'Content-Type: application/json' -X POST \
  localhost:8088/rest/v1/distributionsettypes \
  -d '[{"key":"mcu","name":"MCU firmware","mandatorymodules":[{"id":<firmware-id>}]}]'

# Target types: Zephyr devices take only mcu, Linux devices only os/os_app
curl -u admin:pw -H 'Content-Type: application/json' -X POST \
  localhost:8088/rest/v1/targettypes \
  -d '[{"name":"zephyr","compatibledistributionsettypes":[{"id":<mcu-id>}]},
       {"name":"linux","compatibledistributionsettypes":[{"id":<os-id>},{"id":<os_app-id>}]}]'

raptorctl target type set dev-042 zephyr
raptorctl publish zephyr.signed.bin --version 1.4.1 --module-type firmware --ds-type mcu
```

An **untyped** target accepts any set, so leaving a device untyped removes
the protection. Typing doesn't limit a module to one artifact either, so don't
upload anything else into an MCU module.

## Update size: no delta, but compression

Delta updates aren't available for Zephyr application images today. This was
checked in October 2026 against Zephyr, upstream MCUboot, and Nordic's
`sdk-mcuboot`/`sdk-nrf` main branches:

- The Zephyr hawkBit client always downloads the whole image into slot 1 and
  lets MCUboot swap it in. The only `Range` it sends is `bytes=N-`, to resume
  a download (`CONFIG_HAWKBIT_SAVE_PROGRESS`), so the multi-range serving that
  zchunk relies on goes unused.
- Neither MCUboot nor the nRF Connect SDK patches an application image from a
  diff. NCS's delta DFU (`dfu_target_modem_delta`) is for nRF91 modem firmware
  only, and the modem applies it itself.

The nearest option is **compressed images**: MCUboot in NCS
(`SB_CONFIG_MCUBOOT_COMPRESSED_IMAGE_SUPPORT`, LZMA2 with an ARM thumb
filter) decompresses slot 1 into slot 0 at boot. The download and slot 1 come
out at roughly 70% of the image size. MCUboot has to run in overwrite-only
mode (`SB_CONFIG_MCUBOOT_MODE_OVERWRITE_ONLY`), so a bad image can't be
reverted. The client's own check is only that the artifact fits in slot 1.
To raptor and the hawkBit client the compressed image is just a smaller
artifact, so nothing changes on the server. Upstream MCUboot has the same
`BOOT_DECOMPRESSION` code path, but its decompressor comes from NCS
(`nrf_compress`).

A real delta for MCUs would be a binary diff built against one specific base
version, published as its own set and aimed with a target filter on a
firmware-version attribute. raptor can already serve that. What doesn't
exist yet is the device side: a hawkBit client that writes through a patcher
instead of `flash_img`.

## Verifying against a real device

raptor's test suite drives its DDI API with the Rust `hawkbit` crate and the
JSON-contract test above — not with the Zephyr client itself, which needs
hardware or QEMU. For an end-to-end check, build Zephyr's
`samples/subsys/mgmt/hawkbit` sample against a raptor instance and watch the
server log with `RUST_LOG=raptor=debug,tower_http=debug`: a successful cycle is a
base poll, a `configData` PUT, a `deploymentBase` GET, artifact GETs, then
feedback with `execution: "closed"` — which also logs `update finished` at the
default level.
