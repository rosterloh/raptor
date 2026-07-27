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

`download`/`update` are `forced` for a forced action and `attempt` for a soft
one. The `confirmationBase` response is identical but keyed `confirmation`
instead of `deployment`.

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

Only `closed` is terminal. Posting feedback to a non-active action returns
`410 Gone`.

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
