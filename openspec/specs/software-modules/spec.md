# software-modules Specification

## Purpose

Operator management of software modules and their artifacts through the
Management API: module CRUD, artifact upload and download, and module metadata.

## Requirements

### Requirement: Module identity
A software module SHALL have a name, version and module type key, plus optional vendor and description. Name, version and type together SHALL be unique; a duplicate SHALL be `409`.

#### Scenario: Duplicate module
- **WHEN** an operator creates `rootfs` `1.0` `os` twice
- **THEN** the second create is `409`

### Requirement: Module deletion
Deleting a module SHALL remove its metadata, artifact rows and any blobs left unreferenced. A module that belongs to a distribution set SHALL be refused with `409`, deleting nothing.

#### Scenario: In a set
- **WHEN** an operator deletes a module that a distribution set contains
- **THEN** the response is `409` and the module is unchanged

### Requirement: Artifact upload
`POST /rest/v1/softwaremodules/{id}/artifacts` SHALL accept `multipart/form-data`, stream the file while computing SHA-1, MD5 and SHA-256 in one pass, and return those hashes and the size. Uploads over `max_artifact_size` SHALL be rejected.

#### Scenario: Upload
- **WHEN** an operator uploads `rootfs.img`
- **THEN** the response carries its size, `sha1`, `md5` and `sha256`

### Requirement: Artifact filename validation
An upload whose filename contains `"`, `\`, `/` or a control character SHALL be rejected with `400`.

#### Scenario: Path in filename
- **WHEN** the multipart filename is `../etc/passwd`
- **THEN** the response is `400`

### Requirement: Artifact quota
A module SHALL hold at most `[quota] max_artifacts_per_software_module` artifacts (default 50). Exceeding it SHALL be `429`.

#### Scenario: Quota full
- **WHEN** a module already holds the maximum and another file is uploaded
- **THEN** the response is `429` with `hawkbit.server.error.quota.tooManyEntries`

### Requirement: Artifact list, download and delete
Operators SHALL list a module's artifacts, fetch one, download its bytes from `.../artifacts/{aid}/download`, and delete it.

#### Scenario: Operator download
- **WHEN** an operator GETs `.../artifacts/1/download`
- **THEN** the original bytes are returned

### Requirement: Module metadata
Modules SHALL support key/value metadata (create as array, list, get, update, delete). An entry with `targetVisible` true SHALL be shown to devices in the deployment chunk.

#### Scenario: Visible signature
- **WHEN** a module has metadata `signature` with `targetVisible: true`
- **THEN** devices see it in the chunk's `metadata`
