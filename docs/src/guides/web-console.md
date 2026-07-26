# Web Console

raptor ships an optional web console — a [Dioxus](https://dioxuslabs.com/) WASM
single-page app — embedded directly in the binary.

## Enabling it

The console is behind the `embed-ui` Cargo feature. The Debian package and the
release binaries are built with it on; if you build from source, include the
feature (see [Installation](../getting-started/installation.md)):

```console
$ dx build --release --package raptor-ui
$ cargo build --release --features embed-ui
```

Without the feature the server runs identically but does not serve the UI routes.

## Accessing it

Browse to `/ui`:

```
http://localhost:8080/ui
```

Log in with the same admin credentials as the Management API. The console
authenticates via `POST /rest/v1/login` and holds a session cookie, so you don't
re-enter credentials on every request.

## What it covers

The console surfaces the core read/observe workflow and common actions:

- a dashboard (polling the actions feed and active rollouts),
- targets and target detail,
- [target filters and auto-assignment](target-filters.md),
- distribution sets and detail,
- software modules and detail,
- rollouts and rollout detail,
- the actions feed.

### Rollouts

The rollouts list shows each rollout's status and a progress bar of its targets;
the detail page adds the deploy groups, each with its own bar and a legend of
targets per status (finished / running / error / cancelled / scheduled / not
started), refreshed by the same 5s polling as the rest of the console.

Lifecycle buttons — **Start**, **Pause**, **Resume**, **Delete** — appear for the
transitions the rollout's current status allows, so an operator can drive a
rollout end to end without touching the API. Creating a rollout is still
API-only; see the [Rollouts guide](rollouts.md).

> **Note:** The console tracks the API and lags it slightly. A page for the
> confirmation flow is planned (tracked as an issue on GitHub). Anything not yet
> in the UI is always available through the
> [Management API](../reference/management-api.md).
