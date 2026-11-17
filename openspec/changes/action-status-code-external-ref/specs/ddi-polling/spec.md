# Spec Delta

## MODIFIED Requirements

### Requirement: Attribute upload modes
`PUT .../configData` SHALL accept `{"mode","data"}` with mode `merge` (default; upsert keys), `replace` (drop all, then set) or `remove` (delete the listed keys). Unknown body fields SHALL be ignored. A key or value longer than 128 characters SHALL be rejected with `400` and nothing from the request stored. An attribute change SHALL re-evaluate auto-assign filters.

#### Scenario: Replace
- **WHEN** a target with attributes `{a,b}` PUTs `{"mode":"replace","data":{"c":"1"}}`
- **THEN** its attributes are exactly `{c: "1"}`

#### Scenario: Over-long value
- **WHEN** a device PUTs an attribute value of 129 characters
- **THEN** the request fails with `400` and no attribute changes
