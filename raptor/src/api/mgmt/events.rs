//! `GET /rest/v1/events`: raptor-only Server-Sent Events feed of live updates.

use crate::api::mgmt::targets;
use crate::entity::action;
use crate::error::AppError;
use crate::events::Event;
use crate::state::AppState;
use axum::Router;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::response::sse::{self, KeepAlive, Sse};
use axum::routing::get;
use futures::stream;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use serde::Deserialize;
use std::collections::{HashSet, VecDeque};
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;

pub fn routes() -> Router<AppState> {
    Router::new().route("/rest/v1/events", get(stream))
}

#[derive(Deserialize)]
pub struct EventsQuery {
    target: Option<String>,
    rollout: Option<i64>,
}

#[derive(Clone)]
struct Filter {
    target: Option<String>,
    rollout: Option<i64>,
}

impl Filter {
    fn matches(&self, ev: &Event) -> bool {
        (self.target.is_some() || !ev.is_data())
            && self
                .target
                .as_deref()
                .is_none_or(|c| ev.controller_id() == Some(c))
            && self.rollout.is_none_or(|r| ev.rollout_id() == Some(r))
    }
}

fn to_sse(ev: &Event) -> sse::Event {
    let out = sse::Event::default().event(ev.name());
    let json = match ev {
        Event::Target(e) => serde_json::to_string(e),
        Event::Action(e) => serde_json::to_string(e),
        Event::Rollout(e) => serde_json::to_string(e),
        Event::Download(e) => serde_json::to_string(e),
        Event::Progress(e) => serde_json::to_string(e),
    };
    out.data(json.unwrap_or_else(|_| "{}".into()))
}

pub async fn stream(
    State(st): State<AppState>,
    Query(q): Query<EventsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let filter = Filter {
        target: q.target.clone(),
        rollout: q.rollout,
    };
    let mut pending: VecDeque<sse::Event> = VecDeque::new();
    if let Some(cid) = &q.target {
        targets::find_by_cid(&st.db, cid).await?;
    }
    // Subscribe before the snapshot so nothing published in between is lost.
    let rx = st.events.subscribe();
    let shutdown = st.events.shutdown_signal();
    if let Some(cid) = &q.target {
        // Progress table is bounded only by pruning actions that are over;
        // prune just this target's entries, in one query.
        let snap = st.events.snapshot(cid);
        let mut ids: Vec<i64> = snap.iter().filter_map(Event::action_id).collect();
        ids.sort_unstable();
        ids.dedup();
        let active: HashSet<i64> = if ids.is_empty() {
            HashSet::new()
        } else {
            action::Entity::find()
                .select_only()
                .column(action::Column::Id)
                .filter(action::Column::Id.is_in(ids.clone()))
                .filter(action::Column::Active.eq(true))
                .into_tuple::<i64>()
                .all(&st.db)
                .await?
                .into_iter()
                .collect()
        };
        for id in ids.iter().filter(|id| !active.contains(id)) {
            st.events.clear_action(*id);
        }
        pending.extend(
            snap.iter()
                .filter(|e| e.action_id().is_some_and(|id| active.contains(&id)))
                .filter(|e| filter.matches(e))
                .map(to_sse),
        );
    }
    let s = stream::unfold(
        (rx, shutdown, pending, filter),
        |(mut rx, mut shutdown, mut pending, filter)| async move {
            loop {
                if let Some(ev) = pending.pop_front() {
                    return Some((Ok::<_, Infallible>(ev), (rx, shutdown, pending, filter)));
                }
                let next = tokio::select! {
                    r = rx.recv() => r,
                    _ = shutdown.wait_for(|v| *v) => return None,
                };
                match next {
                    Ok(ev) if filter.matches(&ev) => pending.push_back(to_sse(&ev)),
                    Ok(_) => {}
                    Err(RecvError::Lagged(_)) => {
                        pending.push_back(sse::Event::default().event("resync").data("{}"))
                    }
                    Err(RecvError::Closed) => return None,
                }
            }
        },
    );
    // Tells nginx not to buffer the stream.
    Ok((
        [("x-accel-buffering", "no")],
        Sse::new(s).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))),
    ))
}
