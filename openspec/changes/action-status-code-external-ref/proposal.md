# Proposal: status codes, externalRef and attribute length limits

## Why

Three hawkBit behaviours raptor doesn't mirror yet, all small and all in the
action/DDI-input area:

- Devices can send a numeric `status.code` with feedback. hawkBit stores it on
  the status entry and keeps the latest as the action's `lastStatusCode`,
  filterable with `q=lastStatusCode==N`. raptor drops it (#216). It is also the
  prerequisite for a status-code catalogue (#217) and code-based retries (#218).
- hawkBit 1.2 accepts an optional `externalRef` on DS assignment bodies and
  stores it on the action (#212).
- hawkBit limits each controller attribute key and value to 128 characters;
  raptor stores any length (#210).

## What Changes

- DDI deployment and cancel feedback accept an optional integer `status.code`.
  It is stored on the status entry; every device report replaces the action's
  `lastStatusCode`, with none when the report carried none, as hawkBit does.
- `ActionRest` gains `lastStatusCode` and `externalRef`, `ActionStatusRest`
  gains `code`; all omitted when unset. Action FIQL accepts `lastStatusCode`
  and `externalRef`.
- `POST /rest/v1/targets/{cid}/assignedDS` accepts `externalRef` (at most 512
  characters, hawkBit's limit, else `400`). raptor has no
  `POST /distributionsets/{id}/assignedTargets`, so that body is out of scope.
- `PUT .../configData` rejects a key or value over 128 characters with `400`,
  storing nothing from that request.
- The console's action history, `raptorctl action status` and the TUI history
  pane show the code.

## Impact

- Migration adding `action.last_status_code`, `action.external_ref` and
  `action_status.code` (nullable, SQLite and Postgres).
- `raptor-api-types` (additive fields), `domain/deployment.rs`, DDI
  `feedback.rs`/`config_data.rs`, mgmt `actions.rs`, console, CLI/TUI.
- Specs: `ddi-deployment`, `deployment-actions`, `ddi-polling`.
