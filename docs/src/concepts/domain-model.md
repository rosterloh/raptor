# Domain Model

The entities below are the nouns of raptor. Understanding how they relate makes
the API and the update lifecycle straightforward.

```text
SoftwareModuleType        DistributionSetType
        │                         │
        ▼                         ▼
  SoftwareModule ──< DsModule >── DistributionSet
        │                              │
        ▼                              │ assigned / installed
    Artifact                          ▼
   (blob on disk)     Target ──< Action >── (a deployment)
        ▲                │            │
        │                ▼            ▼
   content-addressed  TargetAttribute  ActionStatus
                                        (+ messages)
              Rollout ──< RolloutGroup >── RolloutTargetGroup
              TargetFilter (optional auto-assign DS)
```

## Core entities

- **Target** — a device, keyed by unique `controllerId`. Holds a security token,
  an `updateStatus` (`unknown` → `registered` → `pending` → `in_sync` / `error`),
  the last poll time and request address, an `auto_confirm` flag, and pointers to
  its assigned and installed distribution sets.
- **TargetAttribute** — key/value pairs a device reports via DDI `configData`.
- **SoftwareModule** — a named, versioned unit of a seeded type (`os`,
  `firmware`, `runtime`, `application`). Owns artifacts.
- **Artifact** — a file (filename, size, sha1/md5/sha256) belonging to one
  module. The **blob** is stored once per sha256 on disk; artifact rows are
  references to it.
- **DistributionSet** — a named, versioned bundle of modules of a seeded type
  (`os`, `os_app`, `app`). `complete` when it has the modules its type requires;
  only complete sets are assignable.
- **Action** — one deployment of a DS to a target. Carries a `status` (the action
  state machine), an `active` flag, and a `forced`/soft indicator. May belong to
  a rollout group. **Invariant: at most one active action per target.**
- **ActionStatus** — an append-only history row (with optional messages) written
  on every device feedback and every server-side transition.

## Rollout entities

- **Rollout** — a staged deployment of a DS to the targets matched by a FIQL
  filter, split into groups, with success/error thresholds.
- **RolloutGroup** — one stage; has its own status and thresholds.
- **RolloutTargetGroup** — the membership join: which targets are in which group
  (a static snapshot taken at creation).

## Target filters

- **TargetFilter** — a saved FIQL query with an optional attached auto-assign
  distribution set and action type. Drives
  [auto-assignment](../guides/target-filters.md).

## Seeded types

Software-module and distribution-set types are **read-only** in this version,
created by the initial migration:

- Software-module types: `os`, `firmware`, `runtime`, `application`
- Distribution-set types: `os`, `os_app`, `app`

## Tenancy

There is no tenant column. raptor is single-tenant; the DDI tenant URL segment is
accepted and ignored, and all generated links use `DEFAULT`.
