# Your First Deployment

This walks through a complete update cycle: create the content, assign it, and
watch a device install it. It uses `curl` for both the operator (Management API)
and the device (DDI API) sides.

Assume raptor is running on `localhost:8080` with admin `admin:yourpassword` and
`ddi.anonymous = true` (so we can poll without a device token).

## 1. Create a software module

A **software module** is a named, versioned unit of a given type (`os`,
`firmware`, `runtime`, or `application`).

```bash
curl -u admin:yourpassword -X POST localhost:8080/rest/v1/softwaremodules \
  -H 'Content-Type: application/json' \
  -d '[{"name":"rootfs","version":"1.0","type":"os"}]'
# -> [{"id":1, ...}]
```

## 2. Upload an artifact

Artifacts are uploaded as multipart form data. raptor streams the bytes to disk
and computes the SHA-1, MD5, and SHA-256 hashes as it goes.

```bash
curl -u admin:yourpassword -X POST localhost:8080/rest/v1/softwaremodules/1/artifacts \
  -F 'file=@rootfs.img'
# -> {"id":1,"providedFilename":"rootfs.img","size":..., "hashes":{...}}
```

## 3. Compose a distribution set

A **distribution set (DS)** bundles one or more modules into a releasable unit. A
DS is `complete` once it has the modules its type requires.

```bash
curl -u admin:yourpassword -X POST localhost:8080/rest/v1/distributionsets \
  -H 'Content-Type: application/json' \
  -d '[{"name":"release","version":"1.0","type":"os","modules":[{"id":1}]}]'
# -> [{"id":1,"complete":true, ...}]
```

## 4. Assign the DS to a target

You do not need to create the target first — an unknown controller ID is
auto-registered on its first poll. Assigning creates an **action** (the record of
one deployment).

```bash
curl -u admin:yourpassword -X POST localhost:8080/rest/v1/targets/my-device/assignedDS \
  -H 'Content-Type: application/json' -d '{"id":1,"type":"forced"}'
# -> {"assigned":1,"alreadyAssigned":0,"total":1,"assignedActions":[{"id":1}]}
```

## 5. Device poll

The device polls the DDI root. Because an action is pending, the response carries
a `deploymentBase` link.

```bash
curl localhost:8080/DEFAULT/controller/v1/my-device
# _links.deploymentBase -> .../deploymentBase/1
```

## 6. Fetch the deployment

```bash
curl localhost:8080/DEFAULT/controller/v1/my-device/deploymentBase/1
```

The response describes the download/update modes and lists each module's
artifacts with hashes, sizes, and download links. A real client (SWUpdate, RAUC)
downloads the artifacts from those links (which support HTTP Range for resume).

## 7. Report feedback

The device reports progress, then a final result. Feedback drives the action
state machine.

```bash
# progress
curl -X POST localhost:8080/DEFAULT/controller/v1/my-device/deploymentBase/1/feedback \
  -H 'Content-Type: application/json' \
  -d '{"status":{"execution":"proceeding","result":{"finished":"none"}}}'

# success
curl -X POST localhost:8080/DEFAULT/controller/v1/my-device/deploymentBase/1/feedback \
  -H 'Content-Type: application/json' \
  -d '{"status":{"execution":"closed","result":{"finished":"success"}}}'
```

On `closed`/`success` the action becomes `finished` and the target's
`updateStatus` becomes `in_sync`. A subsequent poll no longer offers a
`deploymentBase`, and `installedBase` now reflects the installed DS.

## 8. Verify

```bash
curl -u admin:yourpassword localhost:8080/rest/v1/targets/my-device
# "updateStatus": "in_sync"

curl -u admin:yourpassword localhost:8080/rest/v1/targets/my-device/actions
# the action is "finished"
```

That's a full cycle. From here, explore [Rollouts](../guides/rollouts.md) to
stage a deployment across many devices, or
[Target Filters](../guides/target-filters.md) to assign automatically.
