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

## Developing the console

For UI work, `dx build` + `cargo build --features embed-ui` is too slow to
iterate with — every visual tweak pays a full wasm rebuild. Instead, run the
two halves of the app separately and let `dx serve` handle the frontend:

```console
$ cargo run -- serve --config raptor.toml     # the API, on :8088
$ dx serve --package raptor-ui                # the console, on its own port
```

`dx serve` hosts the console on its own port (`:8080` by default) rather than
`:8088`, but `raptor-ui/Dioxus.toml` already proxies `/rest/*` to
`http://localhost:8088/rest` (`[[web.proxy]]`), so the browser only ever talks
to the `dx serve` origin — the login flow's `SameSite=Strict` session cookie
and every other same-origin API call work unmodified, no extra config needed.
Browse to `http://localhost:<dx-port>/ui` and log in as usual. `dx serve`
already gives you rsx hot-reload (markup/style edits apply without a rebuild);
a `.rs` logic change triggers an incremental rebuild, faster than a `dx build`
but still a rebuild.

Dioxus 0.7 also ships `--hotpatch` (`dx serve --hotpatch --package raptor-ui`),
which is meant to patch Rust logic changes into the running app without any
rebuild via the Subsecond engine. As of the pinned `dioxus-cli` 0.7.10, this
did not work for this crate in testing: the initial build succeeds and serves,
but the browser fails to boot the app with
`TypeError: WebAssembly.instantiate(): Import #0 "__wbindgen_placeholder__": module is not an object or function`
— reproduced on repeated clean runs. Plain `dx serve` (no `--hotpatch`) has no
such issue. Until this is resolved upstream, use plain `dx serve` for the fast
loop and fall back to a full `dx build` for a release check. If `dx serve`
itself gets confused (stale rebuild, weird state), press `r` in its terminal
UI to force a full rebuild.

## Accessing it

Browse to `/ui`:

```
http://localhost:8088/ui
```

Log in with the same admin credentials as the Management API. The console
authenticates via `POST /rest/v1/login` and holds a session cookie, so you don't
re-enter credentials on every request.

## What it covers

The console surfaces the core read/observe workflow and common actions:

- a dashboard (fleet counters, the actions feed, active rollouts, and the
  server's configuration),
- targets and target detail,
- [target filters and auto-assignment](target-filters.md),
- [tags](tags.md) — create and edit target and distribution-set tags, tag an
  entity from its detail page, and filter the target and distribution lists by
  tag,
- distribution sets and detail,
- software modules and detail,
- rollouts and rollout detail,
- the actions feed.

### Dashboard

The counter tiles — targets, in sync, pending, error, running actions — come
from `GET /rest/v1/system/statistics`, so the server counts the whole fleet and
the numbers stay correct however large it grows. The active-rollout and
recent-action lists below them are feeds rather than counts and read the
ordinary list endpoints. Everything refreshes on the console's 5s polling.

The **System configuration** card at the foot of the dashboard shows the tenant
configuration devices see (`GET /rest/v1/system/configs`): the polling interval,
whether the confirmation flow is on, and which authentication modes are enabled.
It is read-only — raptor takes its configuration from `raptor.toml`, and the API
answers writes to these keys with `403`. See
[Configuration](../reference/configuration.md) to change any of them.

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
