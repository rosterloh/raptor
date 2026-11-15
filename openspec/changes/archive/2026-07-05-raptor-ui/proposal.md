# Proposal: web console

## Why

Day-to-day operation meant scripting curl against the Management API. Operators
need a console covering the whole update workflow, without breaking the "one
binary, one config" deployment.

## What Changes

- A Dioxus (WASM) single-page console, embedded in the server behind the
  `embed-ui` feature and served at `/ui`.
- Session login for browsers: `POST /rest/v1/login` / `logout` and
  `GET /rest/v1/session`; the Management API accepts a session cookie or Basic.
- A shared `raptor-api-types` crate for Management API DTOs, used by both the
  server and the console.
- Pages: dashboard, targets, distribution sets, software modules (with artifact
  upload), actions.

## Capabilities

### New Capabilities

Imported history: described today by `web-console` and the session parts of
`mgmt-auth`.

### Modified Capabilities

None.

## Impact

New crates `raptor-ui` and `raptor-api-types`; new `embed-ui` feature on
`raptor`. See `design.md` and `plan.md`.
