# live-events Specification

## Purpose

A server-sent event stream (raptor extension) that tells Management API clients
when resources change and carries live download and progress data that has no
REST source.

## Requirements

### Requirement: Event stream endpoint
`GET /rest/v1/events` SHALL stream `text/event-stream` under normal Management API auth, including the session cookie. Optional `target={controllerId}` and `rollout={id}` filters SHALL be combinable. An unknown target SHALL be `404`.

#### Scenario: Unknown target filter
- **WHEN** a client subscribes with `?target=nope`
- **THEN** the response is `404`

### Requirement: Event types
Events SHALL be `target`, `action`, `rollout` (change notices), `download` and `progress` (data), and `resync`, each with camelCase JSON `data`. A `rollout` filter SHALL deliver that rollout's `rollout` events and the `action` events whose `rolloutId` matches.

#### Scenario: Action notice
- **WHEN** a device posts feedback on action 7 of `dev-1`
- **THEN** subscribers matching `dev-1` receive `event: action` with `controllerId` and `actionId`

### Requirement: Data events only for target subscriptions
`download` and `progress` events SHALL be sent only to subscriptions with a `target` filter. On connect, such a subscription SHALL first receive the latest `download`/`progress` for each matching in-flight action.

#### Scenario: Fleet-wide stream
- **WHEN** a client subscribes with no filter
- **THEN** it receives no `download` or `progress` events

### Requirement: Delivery semantics
A keep-alive comment SHALL be sent every 15 s. There SHALL be no event ids or replay. A lagging subscriber SHALL get `resync`. Events SHALL cover one raptor instance, and responses SHALL carry `X-Accel-Buffering: no`.

#### Scenario: Slow consumer
- **WHEN** a subscriber falls behind the in-process buffer
- **THEN** it receives `resync` and should refetch everything it shows
