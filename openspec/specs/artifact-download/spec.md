# artifact-download Specification

## Purpose

How devices list and fetch artifact bytes over DDI: the artifact list, resumable
and multi-range downloads, the `.MD5SUM` companion, and which link URLs are
advertised.

## Requirements

### Requirement: Artifact list
`GET .../softwaremodules/{moduleId}/artifacts` SHALL list the module's artifacts with `filename`, `size`, `hashes` (`sha1`, `md5`, `sha256`) and download links.

#### Scenario: List
- **WHEN** a device lists a module it is entitled to
- **THEN** each artifact carries its three hashes and size

### Requirement: Single-range resume
The download endpoint SHALL honour `Range: bytes=a-b`, `bytes=a-` and suffix `bytes=-n` with a single-part `206` carrying `Content-Range`.

#### Scenario: Resume
- **WHEN** a device requests `bytes=1000-` of a 5000-byte artifact
- **THEN** the response is `206` with `Content-Range: bytes 1000-4999/5000`

### Requirement: Multi-range responses
Several ranges SHALL be answered with one `206 multipart/byteranges`, parts in request order, merging a range that overlaps or directly follows the previous one. If merging leaves one range, a plain single-part `206` SHALL be sent.

#### Scenario: zchunk delta fetch
- **WHEN** SWUpdate requests three disjoint ranges
- **THEN** the response is one multipart `206` with three parts in that order

### Requirement: Unsatisfiable and ignored ranges
A range set that overlaps nothing SHALL get `416` with `Content-Range: bytes */<size>`. A malformed header, a non-`bytes` unit, more than 64 parts after merging, or ranges totalling more than the artifact size SHALL be ignored and the whole artifact sent with `200`.

#### Scenario: Past the end
- **WHEN** a device requests `bytes=9000-` of a 5000-byte artifact
- **THEN** the response is `416`

### Requirement: MD5SUM companion
`GET .../artifacts/{filename}.MD5SUM` SHALL return the artifact's MD5 in md5sum file format.

#### Scenario: Verify
- **WHEN** a device fetches `rootfs.img.MD5SUM`
- **THEN** the body is `<md5>  rootfs.img`

### Requirement: Download link schemes
`download-http`/`md5sum-http` SHALL use `[ddi] artifact_http_url` when set, else the base `url` whatever its scheme. `download`/`md5sum` SHALL appear only when the base `url` is `https`.

#### Scenario: TLS-only deployment
- **WHEN** `url` is `https://ota.example.com` and `artifact_http_url` is unset
- **THEN** both link families point at `https://ota.example.com`

### Requirement: Downloads are served by raptor
Devices SHALL always download through raptor, never directly from the storage backend, so DDI auth, Range handling and download progress events behave the same for every backend.

#### Scenario: S3 backend
- **WHEN** artifacts are stored in S3
- **THEN** the device's download URL is still a raptor DDI URL
