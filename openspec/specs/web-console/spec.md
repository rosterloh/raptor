# web-console Specification

## Purpose

The optional embedded web console at `/ui`: a single-page app that drives
raptor entirely through the public Management API.

## Requirements

### Requirement: Optional embedding
With the `embed-ui` feature, the server SHALL serve the console at `/ui`. Without it, the server SHALL behave identically and not serve UI routes.

#### Scenario: Feature off
- **WHEN** raptor is built without `embed-ui`
- **THEN** `/ui` is not served and every API behaves the same

### Requirement: Public API only
The console SHALL use only public Management API endpoints, never private ones, and authenticate through the session login.

#### Scenario: Login
- **WHEN** an operator submits the login form
- **THEN** the console calls `POST /rest/v1/login` and continues with the session cookie

### Requirement: Coverage
The console SHALL offer a dashboard (statistics, actions feed, active rollouts, read-only system config), targets, target filters with auto-assign, tags, distribution sets, software modules with artifact upload, rollouts with lifecycle and approval buttons, and the actions feed.

#### Scenario: Approve from the console
- **WHEN** a rollout waits for approval
- **THEN** its detail page offers Approve and Deny with an optional remark

### Requirement: Live updates with polling fallback
The console SHALL subscribe to the event stream and refetch on change notices, polling every 30 s while connected (targets list: 5 s). Without the stream it SHALL poll every 5 s. Target pages SHALL show live download and step progress.

#### Scenario: Proxy buffers events
- **WHEN** the event stream cannot connect
- **THEN** pages refresh on 5 s polling as before

### Requirement: Design system
Console changes SHALL follow `DESIGN.md` for colour, type, components and copy.

#### Scenario: New page
- **WHEN** a contributor adds a console page
- **THEN** it uses the components and tokens `DESIGN.md` defines
