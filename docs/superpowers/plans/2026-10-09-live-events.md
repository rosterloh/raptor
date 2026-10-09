# Live events (SSE) and update progress — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stream thin change notices and live download/install progress from raptor to the web console over SSE, so a target's update is visible as it happens.

**Architecture:** An in-process `Events` hub (tokio `broadcast` + an in-memory progress table) on `AppState`. Domain write helpers publish after each DB write; the DDI download and feedback handlers feed progress. `GET /rest/v1/events` serves a filtered snapshot then live events. The console opens one `EventSource` per page, coalesces change notices into ≤1 refetch/s per resource, and renders progress bars; polling remains as a slower fallback.

**Tech Stack:** Rust, axum 0.8 (`response::sse`), tokio `broadcast`/`watch`, `futures` (existing), SeaORM, Dioxus 0.7.10 web, `web-sys` `EventSource`.

**Spec:** `docs/superpowers/specs/2026-10-09-live-events-design.md`

## Global Constraints

- No new crate dependencies. Only new `web-sys` features `EventSource`, `MessageEvent` in `raptor-ui/Cargo.toml`.
- `raptor-api-types` stays serde/serde_json only, wasm32-compatible.
- Do not change hawkBit wire formats. The events endpoint is a raptor extension; DDI `result.progress` is an additive optional field.
- No migration; progress is in memory only.
- Event JSON is camelCase. SSE `event:` names exactly: `target`, `action`, `rollout`, `download`, `progress`, `resync`.
- Broadcast capacity default `1024`; download publish throttle `1s` per `(action_id, filename)` plus always the final chunk; SSE keep-alive `15s`.
- Data events (`download`, `progress`) go only to subscriptions with a `target` filter.
- Publish **after** the DB write.
- `AppState::new` / `with_metrics` signatures unchanged (tests and `main` call them).
- Console: coalesced refetch ≤1 per second per resource; `use_polling` 30 s while connected, 5 s otherwise.
- Keep `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, otel and wasm clippy clean; regenerate `raptor-ui/assets/tailwind.css` with `dx build` if Tailwind classes change.

## Review Focus

- `?target=` for an unknown controllerId → 404 `target`, not an empty stream that looks healthy (Task 3).
- Subscriber disconnects mid-stream → its receiver is dropped; publishing keeps working and `receiver_count` falls (Task 3).
- Download by a target with no active action, or an artifact not in the active action's DS → 200 with the bytes and no event (Task 5).
- Zero-byte artifact → exactly one final `download` with `sent == total == 0` (Task 5).
- Session-cookie auth (what `EventSource` uses) is accepted on `/rest/v1/events` (Task 3).

---

### Task 1: Event DTOs in `raptor-api-types`

**Files:**
- Create: `raptor-api-types/src/events.rs`
- Modify: `raptor-api-types/src/lib.rs` (add `mod events; pub use events::*;`)
- Test: `raptor-api-types/src/tests.rs`

**Interfaces:**
- Produces (all `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`, `#[serde(rename_all = "camelCase")]`):
  - `TargetEvent { controller_id: String }`
  - `ActionEvent { controller_id: String, action_id: i64, #[serde(default, skip_serializing_if = "Option::is_none")] rollout_id: Option<i64> }`
  - `RolloutEvent { rollout_id: i64 }`
  - `DownloadEvent { controller_id: String, action_id: i64, filename: String, sent: u64, total: u64 }`
  - `ProgressEvent { controller_id: String, action_id: i64, cnt: u32, of: u32 }`
  - `pub const EVENT_TARGET: &str = "target"` and likewise `EVENT_ACTION`, `EVENT_ROLLOUT`, `EVENT_DOWNLOAD`, `EVENT_PROGRESS`, `EVENT_RESYNC` (`"resync"`).

- [ ] **Step 1: Write failing tests** `event_payloads_round_trip` using the existing `round_trip` helper:
  - `json!({"controllerId":"dev-1"})` ⇄ `TargetEvent`
  - `json!({"controllerId":"dev-1","actionId":42,"rolloutId":7})` and `json!({"controllerId":"dev-1","actionId":42})` (no `rolloutId` key emitted when `None`) ⇄ `ActionEvent`
  - `json!({"rolloutId":7})` ⇄ `RolloutEvent`
  - `json!({"controllerId":"dev-1","actionId":42,"filename":"rootfs.img","sent":5242880,"total":104857600})` ⇄ `DownloadEvent`
  - `json!({"controllerId":"dev-1","actionId":42,"cnt":2,"of":5})` ⇄ `ProgressEvent`
- [ ] **Step 2:** `cargo test -p raptor-api-types event_payloads_round_trip` → FAIL (unresolved types).
- [ ] **Step 3:** Implement the types and constants.
- [ ] **Step 4:** Same command → PASS; `cargo clippy -p raptor-api-types --target wasm32-unknown-unknown -- -D warnings` clean.
- [ ] **Step 5:** Commit `feat(api-types): live event payloads`.

---

### Task 2: Event hub on `AppState`

**Files:**
- Create: `raptor/src/events.rs` (+ `pub mod events;` in `raptor/src/lib.rs`)
- Modify: `raptor/src/state.rs`
- Test: unit tests in `raptor/src/events.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: Task 1 DTOs.
- Produces:
  - `pub enum Event { Target(TargetEvent), Action(ActionEvent), Rollout(RolloutEvent), Download(DownloadEvent), Progress(ProgressEvent) }` (`Clone, Debug`); `impl Event { pub fn name(&self) -> &'static str; pub fn controller_id(&self) -> Option<&str>; pub fn rollout_id(&self) -> Option<i64>; pub fn is_data(&self) -> bool }`.
  - `pub struct Events` with:
    - `pub fn new(capacity: usize) -> Self`; `impl Default` uses `1024`.
    - `pub fn publish(&self, ev: Event)` — ignores send errors.
    - `pub fn subscribe(&self) -> broadcast::Receiver<Event>`.
    - `pub fn record_download(&self, ev: DownloadEvent)` — always updates the table; publishes only if ≥1 s since the last publish for `(action_id, filename)` **or** `ev.sent == ev.total`.
    - `pub fn record_progress(&self, ev: ProgressEvent)` — updates table, always publishes.
    - `pub fn clear_action(&self, action_id: i64)` — removes all table entries for it.
    - `pub fn snapshot(&self, controller_id: &str) -> Vec<Event>` — current table entries for that target.
    - `pub fn action_ids(&self) -> Vec<i64>` — ids present in the table (for pruning in Task 3).
    - `pub fn shutdown(&self)` and `pub fn shutdown_signal(&self) -> watch::Receiver<bool>` (a `watch` channel set to `true` on shutdown).
  - `Inner` gains `pub events: Events`; `AppState::new`/`with_metrics` use `Events::default()`; new `pub fn with_events(db, cfg, store, events: Events) -> Self` (metrics disabled), for tests.
- Throttle clock: store `std::time::Instant` per key; table is `std::sync::Mutex<HashMap<..>>`, never held across `.await`.

- [ ] **Step 1: Write failing unit tests:**
  - `publish_reaches_subscriber` — subscribe, publish `Target`, `try_recv` yields it.
  - `download_is_throttled_but_final_chunk_always_publishes` — three `record_download` calls for one file within 1 s with `sent` 1, 2, then `sent == total`: subscriber receives exactly the 1st and the 3rd; `snapshot` returns the 3rd.
  - `clear_action_empties_snapshot` — record download + progress, `clear_action`, `snapshot` is empty.
  - `snapshot_is_scoped_to_target` — entries for `dev-1` and `dev-2`; `snapshot("dev-1")` has only `dev-1`.
  - `shutdown_flips_signal` — `*shutdown_signal().borrow()` false, then true after `shutdown()`.
- [ ] **Step 2:** `cargo test -p raptor --lib events` → FAIL.
- [ ] **Step 3:** Implement `events.rs` and the `AppState` changes.
- [ ] **Step 4:** Same command → PASS; `cargo test -p raptor` still green (constructors unchanged).
- [ ] **Step 5:** In `main.rs`, the graceful-shutdown future calls `state.events.shutdown()` before awaiting completion (keep a clone of `state`).
- [ ] **Step 6:** Commit `feat(events): in-process event hub`.

---

### Task 3: `GET /rest/v1/events` SSE endpoint

**Files:**
- Create: `raptor/src/api/mgmt/events.rs`
- Modify: `raptor/src/api/mgmt/mod.rs` (`pub mod events;`, `.merge(events::routes())` inside the auth-layered router)
- Modify: `raptor/tests/common/mod.rs` (SSE helper)
- Test: `raptor/tests/mgmt_events.rs`

**Interfaces:**
- Consumes: `Events::{subscribe, snapshot, action_ids, clear_action, shutdown_signal}`, `Event::{name, controller_id, rollout_id, is_data}`.
- Produces:
  - `pub fn routes() -> Router<AppState>`; handler `stream(State, Query<EventsQuery>) -> Result<Sse<impl Stream<Item = Result<sse::Event, Infallible>>>, AppError>` with `EventsQuery { target: Option<String>, rollout: Option<i64> }`.
  - Test helper `pub async fn sse_next(body: &mut axum::body::Body) -> (String, serde_json::Value)` — reads frames under a 2 s `tokio::time::timeout` until a full `event:`/`data:` message, skipping comment lines; panics on timeout. And `pub async fn sse_none(body: &mut Body, ms: u64)` asserting no message within `ms`.
- Behaviour:
  - `target` given → `targets::find_by_cid` (404 `target` if missing).
  - Matching: `target` filter ⇒ `ev.controller_id() == Some(cid)`; `rollout` filter ⇒ `ev.rollout_id() == Some(id)` (both filters ⇒ both must hold); `is_data()` events require a `target` filter.
  - Snapshot (only with `target`): first prune — for each `action_ids()` whose `action.active` is false or missing, `clear_action`; then emit `snapshot(cid)`.
  - Live: `futures::stream::unfold` over `(Receiver, watch::Receiver)` with `tokio::select!`; `Lagged(_)` ⇒ emit `resync` with data `{}`; `Closed` or shutdown ⇒ end.
  - `Sse::new(..).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))`.

- [ ] **Step 1: Write failing tests** in `mgmt_events.rs` (use `state.events.publish` directly — publish points come in Task 4):
  - `events_requires_auth` — no credentials → 401.
  - `events_accepts_session_cookie` — log in via `/rest/v1/login` as existing login tests do, request with the cookie → 200 and `content-type: text/event-stream`.
  - `unknown_target_filter_is_404` — `?target=nope` → 404.
  - `target_filter_isolates_targets` — two targets; subscribe `?target=dev-1`; publish `Target` for `dev-2` then `dev-1`; `sse_next` → `("target", {"controllerId":"dev-1"})`.
  - `unfiltered_stream_gets_notices_not_data` — publish `Download` then `Target`; `sse_next` → `"target"`.
  - `rollout_filter_matches_action_rollout_id` — `?rollout=7`; publish `Action` with `rollout_id: None`, then `Some(7)`; first message is the `Some(7)` one.
  - `snapshot_sent_on_connect` — create target + active action (assign a DS), `record_download` for it, then subscribe `?target=` → first message `download` with the recorded `sent`.
  - `no_snapshot_for_inactive_action` — `record_download` for an action id that is not active; subscribe → `sse_none(.., 300)`; `action_ids()` no longer contains it.
  - `lagged_subscriber_gets_resync` — state via `AppState::with_events(.., Events::new(2))`; subscribe unfiltered; publish 5 `Target`s before reading; first message `("resync", {})`.
  - `dropped_subscriber_is_released` — subscribe then drop the response body; publish still OK and `state.events` receiver count is back to 0 (expose `pub fn receiver_count(&self) -> usize` on `Events` for this).
- [ ] **Step 2:** `cargo test -p raptor --test mgmt_events` → FAIL.
- [ ] **Step 3:** Implement endpoint, routing and helper.
- [ ] **Step 4:** Same command → PASS.
- [ ] **Step 5:** Commit `feat(mgmt): SSE events endpoint`.

---

### Task 4: Publish change notices from write paths

**Files:**
- Modify: `raptor/src/domain/deployment.rs`, `raptor/src/domain/rollout.rs`, `raptor/src/api/mgmt/actions.rs`, `raptor/src/api/mgmt/distribution_sets.rs`, and every other caller of `add_action_status` (grep)
- Test: `raptor/tests/mgmt_events.rs`

**Interfaces:**
- Changes: `add_action_status(st: &AppState, a: &action::Model, status: &str, messages: &[String]) -> Result<(), AppError>` (was `db, action_id`). After inserting, it publishes `Action { controller_id, action_id: a.id, rollout_id: a.rollout_id }`; `controller_id` resolved by one `target::Entity::find_by_id(a.target_id)` query.
- `set_target_status` publishes `Target { controller_id: t.controller_id }` after its update; so do the direct `update_status = Set(..)` sites (`deployment.rs` assign ~256 and cancel-feedback ~393, `mgmt/actions.rs` ~339, `mgmt/distribution_sets.rs` ~410).
- `set_action(.., active: false)` calls `st.events.clear_action(a.id)`.
- `domain/rollout.rs`: publish `Rollout { rollout_id }` at the end of `create_rollout`, `decide_approval`, `start_rollout`, `pause_rollout`, `resume_rollout`, `stop_rollout`, `delete_rollout`, and inside `evaluate_rollouts` wherever a group or rollout status is set.

- [ ] **Step 1: Write failing tests:**
  - `assign_publishes_target_and_action` — subscribe `?target=dev-1`; POST `assignedDS`; the next two messages (any order) are `target` and `action` with matching ids.
  - `feedback_publishes_action_and_finish_publishes_target` — after assign, DDI feedback `proceeding` with details → `action`; feedback `closed`/`success` → messages include `target`.
  - `rollout_start_publishes_rollout` — create rollout, subscribe `?rollout={id}`, start → `rollout`.
- [ ] **Step 2:** `cargo test -p raptor --test mgmt_events` → new tests FAIL.
- [ ] **Step 3:** Implement; update all `add_action_status` callers to the new signature.
- [ ] **Step 4:** `cargo test -p raptor` → all PASS.
- [ ] **Step 5:** Commit `feat(events): publish target, action and rollout changes`.

---

### Task 5: Download byte counting

**Files:**
- Modify: `raptor/src/api/ddi/artifacts.rs` (`download`)
- Test: `raptor/tests/mgmt_events.rs`

**Interfaces:**
- Consumes: `domain::deployment::active_action`, `Events::record_download`.
- Produces: private `async fn download_owner(st, cid, module_id) -> Result<Option<i64>, AppError>` — active action id iff the target exists, has an active action, and that action's DS contains `module_id` (the same DS-module join `deploymentBase` uses).
- Wrap both the Range and full-file `ReaderStream`s in a counting `map` (`futures::StreamExt`), starting at `start` (Range) or `0`, with `total = a.size as u64`, calling `record_download` per chunk. A zero-length body still records one final event (`sent == total == 0`) before returning. `.MD5SUM` and `download_owner == None` paths are untouched.

- [ ] **Step 1: Write failing tests** (fixture uploads a known-size artifact and assigns its DS):
  - `download_streams_progress_to_target_subscriber` — subscribe `?target=`; GET the artifact and drain it; messages until `download` with `sent == total == size`.
  - `range_download_reports_from_offset` — `Range: bytes=N-`; first `download` has `sent >= N`.
  - `download_without_active_action_publishes_nothing` — no assignment; GET → 200 + full body; `sse_none(.., 300)`.
  - `zero_byte_artifact_reports_complete` — 0-byte artifact; exactly one `download` with `sent == 0 && total == 0`.
- [ ] **Step 2:** `cargo test -p raptor --test mgmt_events download` → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test -p raptor` → PASS (existing `ddi_*` artifact tests unchanged).
- [ ] **Step 5:** Commit `feat(ddi): report artifact download progress`.

---

### Task 6: Accept DDI `result.progress`

**Files:**
- Modify: `raptor/src/api/ddi/feedback.rs`
- Test: `raptor/tests/mgmt_events.rs`

**Interfaces:**
- `FeedbackResult` gains `#[serde(default)] pub progress: Option<FeedbackProgress>`; `#[derive(Deserialize)] pub struct FeedbackProgress { pub cnt: u32, pub of: u32 }`.
- `deployment_feedback`, after `apply_feedback` succeeds and only if the action is still active afterwards (reload or check the execution isn't terminal), calls `st.events.record_progress(ProgressEvent { controller_id: cid, action_id, cnt, of })`.

- [ ] **Step 1: Write failing tests:**
  - `feedback_progress_is_streamed` — feedback `{"status":{"execution":"proceeding","result":{"finished":"none","progress":{"cnt":2,"of":5}}}}` → a `progress` message `{cnt:2, of:5}` (alongside the `action` notice).
  - `feedback_without_progress_still_ok` — existing shape → 200 (already covered by `ddi_*`; assert here too for the extension's sake).
- [ ] **Step 2:** run → FAIL. **Step 3:** implement. **Step 4:** `cargo test -p raptor` → PASS.
- [ ] **Step 5:** Commit `feat(ddi): accept feedback progress`.

---

### Task 7: Console live-events hook and polling fallback

**Files:**
- Create: `raptor-ui/src/components/live.rs` (`pub mod live;` + re-exports in `components/mod.rs`)
- Modify: `raptor-ui/Cargo.toml` (`web-sys` features `EventSource`, `MessageEvent`), `raptor-ui/src/components/mod.rs` (`use_polling_every`), `raptor-ui/src/logic.rs`
- Test: `raptor-ui/src/logic.rs` tests

**Interfaces:**
- Consumes: Task 1 DTOs.
- Produces:
  - `pub enum LiveEvent { Target(TargetEvent), Action(ActionEvent), Rollout(RolloutEvent), Download(DownloadEvent), Progress(ProgressEvent), Resync }`.
  - `pub struct LiveFilter { pub target: Option<String>, pub rollout: Option<i64> }` (`Clone, PartialEq, Default`).
  - `pub fn use_live_events(filter: LiveFilter, on_event: Callback<LiveEvent>) -> Signal<bool>` — returns `connected`. Opens `EventSource` with credentials at `{api base}/rest/v1/events?…`, one listener per event name (closures kept alive for the source's lifetime), `close()` in `use_drop`. Every `open` after the first emits `LiveEvent::Resync`. `readyState == CLOSED` on `error` ⇒ `connected = false`. Off-wasm: returns a `false` signal and does nothing.
  - `pub struct LiveContext(pub Signal<bool>)` provided in `Shell`; `use_live_events` sets it; `use_polling_every` reads it with `.peek()` per tick and waits `30_000` ms instead of `ms` while true.
  - In `logic.rs`: `pub struct Dirty<K>` with `pub fn mark(&mut self, k: K)`, `pub fn take_due(&mut self, now_ms: i64) -> Vec<K>` — returns each marked key at most once per 1000 ms, keeping later marks for the next due tick; and `pub fn progress_label(sent: u64, total: u64) -> String` (`"42% · 5.0 / 100.0 MiB"`; `total == 0` ⇒ `"100%"`).
  - `pub fn use_coalesced_refetch<K: Eq + Hash + Clone + 'static>(on_due: impl FnMut(K) + 'static) -> Callback<K>` in `live.rs` — returned callback marks; a `use_future` loop ticking every 250 ms (`gloo_timers`) calls `on_due` for `take_due(now_ms())`.
- Also update the `use_polling` doc comment: polling is the fallback when live events are disconnected.

- [ ] **Step 1: Write failing host tests:**
  - `dirty_coalesces_within_a_second` — mark A at 0, take_due(0) → [A]; mark A twice at 100, take_due(500) → []; take_due(1000) → [A]; take_due(2500) → [].
  - `progress_label_formats` — `(52_428_800, 104_857_600)` → `"50% · 50.0 / 100.0 MiB"`; `(0, 0)` → `"100%"`.
- [ ] **Step 2:** `cargo test -p raptor-ui` → FAIL.
- [ ] **Step 3:** Implement logic, hook, context, polling change.
- [ ] **Step 4:** `cargo test -p raptor-ui` PASS; `cargo clippy -p raptor-ui --target wasm32-unknown-unknown -- -D warnings` clean.
- [ ] **Step 5:** Commit `feat(ui): live events hook with polling fallback`.

---

### Task 8: Console pages use live events

**Files:**
- Modify: `raptor-ui/src/pages/target_detail.rs`, `rollout_detail.rs`, `targets.rs`, `actions.rs`, `rollouts.rs`, `dashboard.rs`, `raptor-ui/src/pages/shell.rs` (provide `LiveContext`), `raptor-ui/assets/tailwind.css` (regenerated)

**Interfaces:**
- Consumes: `use_live_events`, `use_coalesced_refetch`, `LiveFilter`, `progress_label`.
- Target detail: filter `target = cid()` (re-subscribe when `cid` changes — key the hook's owner component on `cid` or rebuild in an effect). Keys: `enum TargetRes { Target, Actions, History(i64) }`; `target` ⇒ restart `target`, `assigned`, `installed`, `auto_confirm`; `action` ⇒ restart `actions` and mark `History(action_id)`; `Resync` ⇒ all. `ActionRow` takes `history_tick: ReadSignal<u64>` (bumped per due `History(id)`) and reads it in its history resource so an open panel refetches. Progress: `Signal<HashMap<(i64, String), DownloadEvent>>` and `Signal<HashMap<i64, ProgressEvent>>` passed into `ActionRow`, rendered for active actions as one `role="progressbar"` bar per file (`aria-valuenow` = sent, `aria-valuemax` = total, label from `progress_label`) plus `"step {cnt} / {of}"`.
- Rollout detail: filter `rollout = id`; `rollout`/`action`/`Resync` ⇒ coalesced restart of `rollout` and `groups`.
- Lists/dashboard: unfiltered; Targets on `target`, Actions on `action`, Rollouts on `rollout`, Dashboard on any ⇒ coalesced restart of the page's list resource(s).

- [ ] **Step 1:** Implement target detail + `ActionRow` progress.
- [ ] **Step 2:** Implement rollout detail, lists, dashboard.
- [ ] **Step 3:** `dx build --release --package raptor-ui`; commit `raptor-ui/assets/tailwind.css` if changed.
- [ ] **Step 4:** Manual check: `cargo run -- serve --config raptor.toml` + `dx serve --package raptor-ui`; create target/DS, assign, then from a shell download the artifact with `curl --limit-rate 1M` against the DDI URL and post `proceeding`(with `progress`)/`closed` feedback. Expected: bar advances ~1/s, step line appears, header status flips to `in_sync` without reload, network panel shows one `events` request and polling at 30 s. Kill the server → polling returns to 5 s; restart → stream reconnects and page resyncs.
- [ ] **Step 5:** `cargo test -p raptor-ui`, wasm clippy clean. Commit `feat(ui): live target, rollout and list updates`.

---

### Task 9: Stock-client check, docs, changelog, full verification

**Files:**
- Test: `raptor/tests/mgmt_events.rs`
- Modify: `docs/src/reference/management-api.md`, `docs/src/guides/web-console.md`, `docs/src/reference/ddi-api.md`, `CHANGELOG.md`

- [ ] **Step 1: Write test** `hawkbit_client_cycle_with_subscriber` — subscriber on `?target=`; drive poll → deploymentBase → download → feedback `closed` with the `hawkbit` dev-dep client against a served app (follow an existing test that already uses the `hawkbit` crate); assert the client's cycle succeeds and the subscriber sees `download` then `target`.
- [ ] **Step 2:** Run → PASS (no implementation expected; if it fails, fix the regression in the owning task's code).
- [ ] **Step 3:** Docs: endpoint table row + a short "Live events (raptor extension)" section with the event table from the spec in `management-api.md`; live updates + polling fallback in `web-console.md`; `result.progress` accepted in `ddi-api.md`.
- [ ] **Step 4:** `CHANGELOG.md` 1.3.0 **Added**: one bullet for live events endpoint + console live updates/progress, one for DDI `result.progress`, with the issue number (ask the user / open an issue before merging).
- [ ] **Step 5:** Full gate: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo clippy -p raptor --features otel --all-targets -- -D warnings`; `cargo clippy -p raptor-ui --target wasm32-unknown-unknown -- -D warnings`; `cargo test --workspace`; `cargo test -p raptor --features otel --test telemetry`; `mdbook build docs`. All succeed.
- [ ] **Step 6:** Commit `docs: live events and DDI progress`.
