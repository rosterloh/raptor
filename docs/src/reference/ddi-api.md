# DDI API Reference

The device-facing API, under `/{tenant}/controller/v1/{controllerId}`. The
`tenant` segment is accepted and ignored (raptor is single-tenant; links use
`DEFAULT`). Requests are authenticated by target token, gateway token, or
anonymous mode — see [Authentication](../guides/authentication.md).

Response JSON matches the hawkBit DDI v1 schemas field-for-field.

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
installed. `configData` is always present.

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
