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

- a dashboard (polling the actions feed),
- targets and target detail,
- distribution sets and detail,
- software modules and detail,
- the actions feed.

> **Note:** The console tracks the API and lags it slightly. Pages for rollouts,
> target filters, and the confirmation flow are planned (tracked as issues on
> GitHub). Anything not yet in the UI is always available through the
> [Management API](../reference/management-api.md).
