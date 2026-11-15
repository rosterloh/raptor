# artifact-storage Specification

## Purpose

Where artifact bytes live: a content-addressed blob store on local disk or an
S3-compatible bucket, deduplicated by SHA-256, with bytes never passing through
the database.

## Requirements

### Requirement: Content-addressed layout
Blobs SHALL be stored once per SHA-256 under `<sha256[0..2]>/<sha256>`, on disk under `artifact_dir` or in the bucket under an optional `prefix`. Artifact rows SHALL reference blobs by hash.

#### Scenario: Identical uploads
- **WHEN** the same bytes are uploaded to two modules
- **THEN** one blob is stored and two artifact rows reference it

### Requirement: Reference-counted deletion
A blob SHALL be removed from storage only when no artifact row references its hash any more.

#### Scenario: Shared blob
- **WHEN** one of two artifacts sharing a blob is deleted
- **THEN** the blob remains

### Requirement: Backend selection
`[storage] backend` SHALL select `fs` (default) or `s3`. With `s3`, `[storage.s3]` SHALL configure endpoint, bucket, credentials, region, prefix, path-style addressing, an extra CA bundle and multipart part size (minimum 5 MiB).

#### Scenario: Missing section
- **WHEN** no `[storage]` section is present
- **THEN** blobs are stored on local disk

### Requirement: S3 startup check
With the `s3` backend, raptor SHALL `HEAD` the bucket at startup and refuse to serve if that fails.

#### Scenario: Bad credentials
- **WHEN** the S3 secret key is wrong
- **THEN** `raptor serve` exits at startup

### Requirement: Upload spooling
Uploads SHALL be spooled to `<artifact_dir>/.tmp` while hashed, then committed to the backend.

#### Scenario: S3 upload
- **WHEN** a file is uploaded with the `s3` backend
- **THEN** it is hashed locally first, then stored under its hash key

### Requirement: Backfill to S3
`raptor storage-backfill` SHALL copy every local blob the bucket lacks, skip ones it already holds, never modify local files, and be safe to re-run.

#### Scenario: Re-run
- **WHEN** backfill runs twice
- **THEN** the second run uploads nothing that was already copied
