# DDI API Reference

The device-facing API, under `/{tenant}/controller/v1/{controllerId}`. Requests
are authenticated by target token, gateway token, or anonymous mode — see
[Authentication](../guides/authentication.md).

> **Configure clients with tenant `DEFAULT`.** raptor is single-tenant: the
> `tenant` path segment is accepted and ignored, and every emitted link says
> `DEFAULT`. A device configured with anything else (Zephyr:
> `CONFIG_HAWKBIT_TENANT`) still works — it just follows hrefs that disagree with
> its own config, and it would break if multi-tenancy landed later. raptor logs a
> warning the first time it sees a non-`DEFAULT` poll.

Response JSON matches the hawkBit DDI v1 schemas field-for-field.

Handlers live under `raptor/src/api/ddi/`, one module per resource: poll root
in `root.rs`, deployment/installed base in `deployment.rs`, feedback/cancel in
`feedback.rs`, confirmation flow in `confirmation.rs`, and artifacts in
`artifacts.rs`, all wired together in `mod.rs`.

## Endpoints

| Method | Path (under `/{tenant}/controller/v1/{cid}`) | Description |
|---|---|---|
| `GET` | `/` | poll root: `config.polling.sleep` + `_links` |
| `PUT` | `/configData` | report device attributes (merge / replace / remove) |
| `GET` | `/deploymentBase/{actionId}` | the deployment to install |
| `POST` | `/deploymentBase/{actionId}/feedback` | deployment progress/result |
| `GET` | `/confirmationBase/{actionId}` | pending deployment awaiting confirmation |
| `POST` | `/confirmationBase/{actionId}/feedback` | confirm / deny |
| `POST` | `/confirmationBase/activateAutoConfirm` | device enables auto-confirm |
| `POST` | `/confirmationBase/deactivateAutoConfirm` | device disables auto-confirm |
| `GET` | `/cancelAction/{actionId}` | cancellation to acknowledge |
| `POST` | `/cancelAction/{actionId}/feedback` | confirm cancellation |
| `GET` | `/installedBase/{actionId}` | last successfully installed deployment |
| `GET` | `/softwaremodules/{moduleId}/artifacts` | artifact list for a module |
| `GET` | `/softwaremodules/{moduleId}/artifacts/{filename}` | artifact download (HTTP Range) |
| `GET` | `/softwaremodules/{moduleId}/artifacts/{filename}.MD5SUM` | md5sum-file |

## Poll root

```json
{
  "config": { "polling": { "sleep": "00:05:00" } },
  "_links": {
    "configData":       { "href": ".../configData" },
    "deploymentBase":   { "href": ".../deploymentBase/7" }
  }
}
```

Which `_links` appear depends on the target's state: `deploymentBase` when an
action is `running`, `confirmationBase` when it's `wait_for_confirmation`,
`cancelAction` when it's `canceling`, and `installedBase` once something has been
installed.

`configData` appears only while raptor actually wants attributes — a freshly
registered target, or one an operator has re-armed with `requestAttributes` (see
[Management API](management-api.md#targets)). It disappears as soon as a
`configData` PUT arrives. This matters because some clients (the Zephyr hawkbit
client among them) re-upload their entire attribute set on *every* poll that
carries the link, which is wasted uplink on a large or cellular fleet.

## deploymentBase

```json
{
  "id": "7",
  "deployment": {
    "download": "forced",
    "update": "forced",
    "chunks": [
      { "part": "os", "version": "1.0", "name": "rootfs",
        "artifacts": [
          { "filename": "rootfs.img", "size": 12345,
            "hashes": { "sha1": "...", "md5": "...", "sha256": "..." },
            "_links": { "download-http": {"href": "..."},
                        "md5sum-http":  {"href": "..."} } }
        ],
        "metadata": [ { "key": "signature", "value": "sig-1" } ] }
    ]
  },
  "actionHistory": { "status": "RUNNING", "messages": [] }
}
```

`download`/`update` follow the action's type, matching hawkBit's
`calculateDownloadType`/`calculateUpdateType`:

| Action type | `download` | `update` |
|---|---|---|
| `forced` | `forced` | `forced` |
| `soft` | `attempt` | `attempt` |
| `timeforced`, before `forcetime` | `attempt` | `attempt` |
| `timeforced`, after `forcetime` | `forced` | `forced` |
| `downloadonly` | `forced` | `skip` |

The modes are computed per request, so a `timeforced` action flips on its own
once the deadline passes. See [Assignments & Actions](../guides/actions.md) for
the operator side. The `confirmationBase` response is identical but keyed
`confirmation` instead of `deployment`.

A chunk's `metadata` array carries any software-module metadata marked
`targetVisible` (see the Management API). The key is omitted entirely when a
module has no visible metadata.

## Feedback

```json
{ "status": { "execution": "closed", "result": { "finished": "success" } } }
```

- `execution` ∈ `proceeding`, `scheduled`, `resumed`, `downloading`,
  `downloaded`, `canceled`, `rejected`, `closed`.
- `result.finished` ∈ `none`, `success`, `failure`.

`closed` is terminal for every action type. For a `downloadonly` action
`downloaded` is *also* terminal — that is the whole job, so the action closes
with `detailStatus: downloaded` and the target's installed DS is left unchanged.
For all other types `downloaded` is recorded as progress only. Posting feedback
to a non-active action returns `410 Gone`.

Confirmation feedback uses a different body:

```json
{ "confirmation": "confirmed", "details": ["…"] }   // or "denied"
```

## configData

```json
{ "mode": "merge", "data": { "hw": "rev2", "os": "linux" } }
```

`mode` ∈ `merge` (default; upsert keys), `replace` (drop all, then set), `remove`
(delete the listed keys). Extra legacy fields in the body are ignored.

## Artifact download & Range

The artifact download endpoint honors HTTP `Range` (RFC 7233) so an interrupted
download resumes with a `206 Partial Content` response rather than restarting.

### `download-http` vs `download`

Each artifact carries up to two link families, following hawkBit's convention:
`download-http` / `md5sum-http` are the plain-HTTP URLs, `download` / `md5sum`
the HTTPS ones. Clients pick one and take the scheme from the href itself — the
Zephyr hawkbit client uses `download-http`.

raptor only advertises a genuinely different plain-HTTP URL when you have told it
one is reachable, via `[ddi] artifact_http_url`:

```toml
url = "https://ota.example.com"
[ddi]
artifact_http_url = "http://dl.example.com:8080"
```

| Config | `download-http` | `download` |
|---|---|---|
| `url` http, no `artifact_http_url` | the `url` (http) | *(absent)* |
| `url` https, no `artifact_http_url` | the `url` (https) | the `url` (https) |
| `url` https + `artifact_http_url` | `artifact_http_url` (http) | the `url` (https) |

The middle row is why `download-http` can carry an `https://` href: on a TLS-only
deployment there is no plain-HTTP port to point at, and emitting one anyway would
break every client that follows the link. Set `artifact_http_url` when you want
device downloads to bypass TLS.
