# raptor UI — Dioxus web management console

**Date:** 2026-07-05
**Status:** Approved design, pre-implementation

## Purpose

A full management console for raptor: replace the curl workflow end-to-end
(create software modules, upload artifacts, build distribution sets, assign to
targets, monitor actions). Built with Dioxus (WASM SPA), compiled into the
raptor binary and served at `/ui`, preserving the "one binary, one config"
deployment story. The UI consumes only the public Management API — it is a
permanent dogfooding client and never gains private endpoints (single
exception: the login endpoint below).

## Scope

**In:**
- Dioxus web (WASM) SPA with client-side routing, embedded in the raptor
  binary behind a cargo feature, served at `/ui`
- Login page + server-side session (httpOnly cookie); mgmt auth middleware
  accepts session cookie OR basic auth
- Pages: dashboard, targets (list/detail/assign DS/cancel action/delete),
  distribution sets (list/create/assign modules/deploy/delete), software
  modules (list/create/artifact upload/download/delete), global actions list
- Shared `raptor-api-types` crate for Management API DTOs
- Server-side paging + FIQL search via existing `q=`/`offset`/`limit`
- Polling for live-ness (no push)
- Dark-first design palette (Tailwind); light theme deferred

**Out (deferred):**
- Rollouts, tags, target filters — UI grows them when the API does
- Multi-user / roles (single admin, as today)
- CSRF tokens (`SameSite=Strict` covers browser cross-site POSTs in v1)
- SSE/WebSocket live updates
- Light theme polish, i18n, mobile layouts (desktop-width admin tool)
- Headless-browser E2E tests

## Architecture

Two new workspace members alongside `raptor` and `migration`:

```
raptor/
├── raptor/               # server (unchanged role) + embed-ui feature
├── raptor-api-types/     # shared Management API DTOs (serde only, WASM-safe)
├── raptor-ui/            # Dioxus web app (WASM, dioxus-router, Tailwind)
└── migration/
```

- **`raptor-api-types`** — plain serde structs extracted from
  `raptor/src/api/mgmt/dto.rs`: targets, software modules, distribution sets,
  artifacts, actions, the paged-list envelope, the error body, and the login
  request. No sea-orm/axum dependencies; must compile to `wasm32`. The server
  keeps its entity→DTO conversion impls; the UI deserializes the same structs,
  so type drift is a compile error.
- **`raptor-ui`** — Dioxus web SPA. All API access goes through one
  `ApiClient` wrapper (relative base URL, `credentials: include`) returning
  `Result<T, ApiError>` of `raptor-api-types` structs.

### Build & embedding

- `dx build --release` (in `raptor-ui/`) emits static assets.
- raptor gains an **`embed-ui`** cargo feature: assets embedded via
  `rust-embed` and served at `/ui`, with an `index.html` fallback for all
  `/ui/*` paths so client-side routes survive refresh.
- Feature **off** (default): `cargo build` works with no Dioxus toolchain;
  `/ui` returns 404.
- Release build is two documented commands (README): `dx build --release`
  then `cargo build --release --features embed-ui`; release CI runs the same
  two steps. No task runner.
- Dev loop: `dx serve` with a proxy for `/rest` to a locally running raptor —
  hot reload without rebuilding the server.

## Auth & sessions

- **`POST /rest/v1/login`** `{"username","password"}` — verifies against the
  existing `[mgmt]` username + argon2 hash from `raptor.toml`. On success:
  32-byte random token stored server-side; cookie `raptor_session` with
  `HttpOnly`, `SameSite=Strict`, `Path=/`, plus `Secure` when the request
  arrived over TLS. Responds `204 No Content`. Failure responds `401` (argon2
  verification cost doubles as brute-force damping).
- **`POST /rest/v1/logout`** — deletes the session, clears the cookie.
- **Session store** — in-memory `HashMap<token, expiry>` in `AppState`,
  sliding idle expiry of 24h (code constant, not config). Restart logs
  everyone out; acceptable for a single-admin console, avoids a sessions
  table.
- **Middleware** — `mgmt_auth` accepts a valid session cookie first, then
  falls back to basic auth. Existing curl/CI workflows untouched. DDI auth
  untouched.
- **Client** — SPA never reads the token (httpOnly); any `401` triggers a
  global redirect to `/ui/login`. README gets a one-line security note about
  the SameSite-based CSRF stance.

## Pages & navigation

Persistent left sidebar (Dashboard, Targets, Distributions, Modules, Actions)
with logout; content area right. Routes under `/ui`:

- **`/ui/login`** — the only route outside the app shell. Username/password
  form; inline error on bad credentials.
- **`/ui/` Dashboard** — stat tiles (targets by update status: in-sync /
  pending / error / unknown; running-actions count) and a recent-actions feed.
  Computed client-side from list endpoints — fine at single-binary fleet
  sizes.
- **`/ui/targets`** — paged table (name, controller ID, update-status badge,
  last poll time); search box compiling to a FIQL `q=` filter. Row →
  **target detail**: config-data attributes, assigned/installed distribution
  set, action history; write ops: *Assign distribution set* (picker dialog
  with forced/soft), *Cancel action* (running actions), *Delete target*.
- **`/ui/distributions`** — paged table; create dialog (name, version, type);
  detail with assigned modules + *assign modules* picker; *Deploy…* opens the
  assign-to-target dialog; delete.
- **`/ui/modules`** — paged table; create dialog (name, version, type
  os/application); detail with artifact list (filename, size, SHA1, download
  link), drag-or-browse multipart **artifact upload** with progress bar;
  delete artifact/module.
- **`/ui/actions`** — global action table (target, distribution set, status,
  type, created/updated), status filter, cancel for active actions.

**Shared components:** generic paged `DataTable` (server-side paging via
`offset`/`limit`, FIQL search), status badges, confirm dialog, toast stack.

**Styling:** Tailwind, dark palette as the default/base design; light theme is
a later variant, not a v1 deliverable.

## Data flow

- Reads via `use_resource` around `ApiClient` calls; mutations restart the
  affected resource so tables refresh after every write.
- Polling: dashboard, actions page, and any target detail with a running
  action re-fetch every 5 seconds while visible. No push in v1.

## Error handling

- `ApiClient` parses raptor's error body into typed `ApiError`
  (status + message).
- `401` → global redirect to login.
- Failed **reads** → inline error state in the table/panel with retry.
- Failed **writes** → toast with the server's message; dialog stays open so
  input isn't lost. Upload failures name the file.
- `console_error_panic_hook` wired so WASM panics are diagnosable.

## Testing

- **Server (bulk):** axum integration tests in the existing style —
  login/logout/session expiry, cookie-authed mgmt calls, basic-auth
  regression, `/ui` asset serving with SPA fallback.
- **Shared types:** existing mgmt API tests serialize through
  `raptor-api-types`; drift breaks compilation or existing tests.
- **UI:** native-target unit tests for pure logic only — FIQL query
  construction from the search box, paging math, status→badge mapping. No
  browser harness in v1.
- **Manual smoke path (documented, run before release):** login → create
  module → upload artifact → create distribution set → assign modules →
  assign to target → watch action progress → cancel/complete → logout.
