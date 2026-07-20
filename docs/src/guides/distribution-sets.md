# Distribution Sets

A **distribution set (DS)** is the releasable unit you assign to devices: a named,
versioned bundle of software modules.

## DS types

raptor seeds three distribution-set types: `os`, `os_app`, and `app`. Like
module types, they are read-only. The type determines which module types a DS
needs to be considered **complete**.

## Creating a distribution set

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/distributionsets \
  -H 'Content-Type: application/json' \
  -d '[{"name":"release","version":"1.0","type":"os","modules":[{"id":1}]}]'
```

You can pass modules inline (as above) or add them afterward:

```bash
curl -u admin:pw -X POST localhost:8080/rest/v1/distributionsets/1/assignedSM \
  -H 'Content-Type: application/json' -d '[{"id":2}]'
```

## Completeness

A DS is `complete` when it contains the modules its type requires. **Only complete
distribution sets can be assigned or deployed** — assigning an incomplete DS (or
attaching one as a target-filter auto-assignment) returns `400 Bad Request`.

```bash
curl -u admin:pw localhost:8080/rest/v1/distributionsets/1
# {"id":1,"complete":true, "modules":[...], ...}
```

## Listing and filtering

Distribution sets support the usual paging, sorting, and FIQL query parameters:

```bash
curl -u admin:pw 'localhost:8080/rest/v1/distributionsets?q=name==release*&sort=version:DESC'
```

## Lifecycle

Distribution sets are referenced by actions and rollouts. raptor keeps the
content-addressing scheme independent of DS identity, so the same artifact bytes
can back many distribution sets without duplication.
