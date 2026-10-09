# Live events (SSE) and update progress — design

**Date:** 2026-10-09
**Status:** Approved design, not yet implemented
**Branch:** `feat/live-events`

## Purpose

An operator watching a target in the web console should see an update unfold
live — download progress per artifact, the device's own messages, status
transitions — without reloading and without the 5 s polling lag.

Today only list pages, the dashboard, rollout detail and the target page's
action list poll. On the target page the header status, assigned/installed
sets and an expanded status history stay stale until reload, and there is no
progress information at all: raptor's DDI feedback parser silently drops
hawkBit's `result.progress`.

## Decisions

| Question | Decision | Why |
| --- | --- | --- |
| Topology | Single instance; in-process `tokio::sync::broadcast` | raptor is one binary. Multi-instance would need Postgres `LISTEN/NOTIFY` behind the same `Events` API — out of scope |
| Tenancy | Not modelled on events | raptor is single-tenant at runtime (`auth/ddi.rs` 404s other tenants) |
| Consumers | Web console only | The endpoint is generic; raptorctl/TUI can adopt it in a follow-up |
| Progress persistence | Live only, in memory | No migration, no change to hawkBit DTOs; devices re-report after a restart |
| Event shape | Thin invalidations + data-carrying progress (approach A) | REST stays the single source of truth; a lost event costs a refetch, never wrong data. Progress has no REST source, so it carries data |
| Transport | SSE, not WebSocket | One-way, plain HTTP, browser auto-reconnect, works with the mgmt session cookie; no new server dependencies |

## Wire contract (raptor extension — not part of hawkBit)

`GET /rest/v1/events` — `text/event-stream`, behind the normal mgmt auth
(Basic or session cookie; the console uses the cookie because `EventSource`
cannot send `Authorization`).

Query parameters (optional, combinable):

- `target={controllerId}` — only events about that target.
- `rollout={id}` — only events about that rollout or its actions.

Data-carrying events (`download`, `progress`) are sent **only** to
subscriptions with a `target` filter, so fleet-wide downloads never flood an
unfiltered (list/dashboard) stream.

One SSE message per event; `event:` is the type, `data:` is camelCase JSON:

| `event:` | `data:` | Meaning |
| --- | --- | --- |
| `target` | `{"controllerId"}` | update status / assigned / installed set changed |
| `action` | `{"controllerId","actionId","rolloutId"?}` | action status changed, status-history entry added, cancel/confirm |
| `rollout` | `{"rolloutId"}` | rollout or group state/counters changed |
| `download` | `{"controllerId","actionId","filename","sent","total"}` | artifact bytes streamed to the device |
| `progress` | `{"controllerId","actionId","cnt","of"}` | DDI `result.progress` reported by the device |
| `resync` | `{}` | the subscriber lagged; refetch everything shown |

Payload DTOs live in `raptor-api-types` (new `events` module; serde only,
wasm32-compatible) with round-trip tests.

On connect the server first sends a **snapshot**: the latest `download` /
`progress` message for every in-flight action matching the filter. Then live
events. No event ids and no `Last-Event-ID` replay — a reconnect is a resync.
Keep-alive comment every 15 s (axum `KeepAlive`).

## Server

### Event hub — `raptor/src/events.rs`

- `Event` enum (`Target`, `Action`, `Rollout`, `Download`, `Progress`) and
  `Events { tx: broadcast::Sender<Event>, progress: Mutex<HashMap<..>> }`,
  added to `AppState`'s `Inner`. Default channel capacity 1024;
  `Events::new(capacity)` lets tests use a small one (no config knob).
- `publish()` never blocks or errors (no subscribers → dropped).
- Progress table: latest `Download` per `(action_id, filename)` and latest
  `Progress` per `action_id`; `std::sync::Mutex`, never held across `.await`.
  Entries are removed when the action reaches a terminal state
  (finished / error / canceled). Bounded by in-flight actions.

### Publish points

Always **after** the DB write, so a client refetching on the event sees the
new state.

- `domain/deployment.rs`: `update_status` writes (assign, feedback, cancel
  feedback, confirmation) → `Target`; `add_action_status` and action state
  changes → `Action` (with `rolloutId` when set).
- `domain/rollout.rs`: state transitions and group progression → `Rollout`.

### Download byte counting — `api/ddi/artifacts.rs::download`

- Resolve the target's active action via `domain::deployment::active_action`
  and check its DS contains `moduleId`; otherwise publish nothing (no active
  action, `.MD5SUM`, out-of-band fetch).
- Wrap the `ReaderStream` with a counting `map` (existing `futures` dep).
  `sent = range_start + bytes so far`, `total = artifact size`, so a Range
  resume continues instead of restarting at 0. Bytes handed to hyper ≈ bytes
  sent (kernel buffering ignored).
- Per artifact file, not aggregated per action.
- Throttle: at most one `Download` publish per second per file, plus always
  the final chunk. The progress table is updated on every chunk.

### DDI `result.progress` — `api/ddi/feedback.rs`

- `FeedbackResult` gains `#[serde(default)] progress: Option<{cnt: u32, of: u32}>`
  per hawkBit's DDI feedback schema. Clients that omit it are unaffected.
- `apply_feedback` stores it in the progress table and publishes `Progress`.
  Not persisted. `details` messages are persisted exactly as today.
- Which stock clients send it, and whether `cnt/of` means steps or bytes, is
  unverified — the console renders it as "step cnt / of", not a byte bar.

### Endpoint — `api/mgmt/events.rs`

- Subscribes to the broadcast, sends the filtered snapshot, then maps
  `broadcast::Receiver` into an SSE stream via `futures::stream::unfold`.
- `RecvError::Lagged` → emit `resync` and continue. `Closed` → end the stream.
- Streams end on graceful shutdown rather than holding it open.

## Console (`raptor-ui`)

### `use_live_events(filter, on_event)` — `components/live.rs`

- Opens a `web_sys::EventSource` (enable `EventSource`, `MessageEvent` on the
  existing `web-sys` dep) at mount; closes it in `use_drop`.
- Parses each named event into the `raptor-api-types` DTO and calls the page's
  `Callback`. Every `open` after the first, and every `resync`, calls a
  "refetch all" path.
- Exposes a `connected` signal. `use_polling` slows to 30 s while connected
  and returns to 5 s otherwise (closed, older server, buffering proxy) — the
  console is never worse than today.
- Session expiry: `EventSource` can't see a 401; once `CLOSED`, report
  disconnected and let the existing REST 401 redirect handle login.
- No-op off wasm (same pattern as `tab_hidden`), so host tests compile.

### Coalesced refetch

Change notices only mark resources dirty; a small loop restarts each dirty
resource at most once per second. Restarting keeps the previous value
(no suspense), so nothing flashes.

### Pages

| Page | Filter | Handling |
| --- | --- | --- |
| Target detail | `target={cid}` | `target` → target, assigned, installed, auto-confirm. `action` → actions + open history panels. `download`/`progress` → `Signal<HashMap<(action_id, filename), ..>>` rendered by `ActionRow` as per-file bars plus "step cnt / of" |
| Rollout detail | `rollout={id}` | `rollout`/`action` → rollout + groups |
| Targets, Actions, Rollouts lists; Dashboard | none | matching change notice → coalesced list refetch |

Progress bars use `role="progressbar"` with `aria-valuenow`/`aria-valuemax`.
No toasts or live-region announcements for status churn.

## Testing

- `raptor/tests/mgmt_events.rs` (frames read with `http-body-util`, each under
  `tokio::time::timeout`):
  - assign → `target` + `action`; feedback with details → `action`
  - target filter isolates `dev-1` from `dev-2`; unfiltered gets no data events
  - artifact download → `download` events ending at `sent == total`;
    `Range: bytes=N-` → first event reports `sent >= N`
  - feedback with `result.progress` → `progress`; without it → still 200
  - snapshot on connect mid-download; no snapshot after the action finishes
  - unauthenticated → 401
  - tiny channel capacity → overflow yields `resync`
- Full DDI cycle with the `hawkbit` client crate (existing dev-dep) while a
  subscriber is connected — DDI unchanged.
- Round-trip JSON tests for every event DTO in `raptor-api-types/src/tests.rs`.
- Console: host unit tests in `logic.rs` for coalescing bookkeeping and
  progress formatting; `EventSource` wiring verified manually with `dx serve`
  and a curl-simulated device (stated in the PR).

## Documentation

- `docs/src/reference/management-api.md`: the endpoint, marked as a raptor
  extension with no hawkBit equivalent.
- `docs/src/guides/web-console.md`: live updates and the polling fallback.
- `docs/src/reference/ddi-api.md`: `result.progress` is accepted.
- `CHANGELOG.md` under 1.3.0 **Added**.

## Out of scope

- Multiple raptor instances (Postgres `LISTEN/NOTIFY`).
- raptorctl / TUI consumption.
- Persisting progress or exposing it on REST payloads.
- Per-action aggregated download percentage.
