# Proposal: live events and update progress

## Why

An operator watching a target in the console could not see an update unfold:
pages polled every 5 s, several views stayed stale until reload, and the DDI
feedback parser dropped hawkBit's `result.progress`.

## What Changes

- `GET /rest/v1/events`: a server-sent event stream (raptor extension) of change
  notices and live download/progress data, filterable by target and rollout.
- Download byte counting on DDI artifact downloads.
- DDI feedback accepts `result.progress` and publishes it, without persisting it.
- The console subscribes to the stream and falls back to polling without it.

## Capabilities

### New Capabilities

Imported history: described today by `live-events`, plus the progress parts of
`ddi-deployment` and `web-console`.

### Modified Capabilities

None.

## Impact

New `events` module and mgmt route; changes to DDI download and feedback; new
event DTOs in `raptor-api-types`; console `use_live_events` hook. See
`design.md` and `plan.md`.
