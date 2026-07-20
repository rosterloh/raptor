# Software Modules & Artifacts

A **software module** is a named, versioned unit of updatable content. An
**artifact** is a file belonging to a module (the actual firmware image, package,
etc.).

## Module types

raptor seeds four software-module types: `os`, `firmware`, `runtime`, and
`application`. Types are **read-only** in raptor (created via migration); you
reference them by key when creating a module.

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/softwaremodules \
  -H 'Content-Type: application/json' \
  -d '[{"name":"rootfs","version":"1.0","type":"os","vendor":"ACME"}]'
```

The combination of name + version + type is unique; a duplicate returns `409
Conflict`.

## Uploading artifacts

Artifacts are uploaded as `multipart/form-data`. raptor streams the upload to
disk while computing the SHA-1, MD5, and SHA-256 hashes in one pass:

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/softwaremodules/1/artifacts \
  -F 'file=@rootfs.img'
```

The response includes the computed hashes and the byte size. Maximum upload size
is governed by `max_artifact_size` (see the
[Configuration Reference](../reference/configuration.md)).

## Content-addressed storage

Blobs are stored **once per SHA-256**, laid out git-object-style:

```
<artifact_dir>/<sha256[0..2]>/<sha256>
```

Uploading identical content twice stores the bytes once and adds a second
reference; the blob is removed from disk only when the last artifact row
referencing it is deleted. This dedup is transparent — each module still sees its
own artifact row with its own filename.

## Listing, downloading, deleting

```bash
# list a module's artifacts
curl -u admin:pw localhost:8080/rest/v1/softwaremodules/1/artifacts

# download (operator side)
curl -u admin:pw -O localhost:8080/rest/v1/softwaremodules/1/artifacts/1/download

# delete
curl -u admin:pw -X DELETE localhost:8080/rest/v1/softwaremodules/1/artifacts/1
```

## Device-side download

Devices fetch artifacts through the DDI API, not the Management API:

```
GET /{tenant}/controller/v1/{cid}/softwaremodules/{moduleId}/artifacts/{filename}
```

This endpoint supports **HTTP Range** requests (RFC 7233), so an interrupted
download resumes rather than restarting — important for large firmware images
over flaky links. A companion `{filename}.MD5SUM` endpoint returns the
md5sum-file format some clients verify against.
