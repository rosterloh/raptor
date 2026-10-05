//! TUI application state and the async result channel. Every network call
//! runs on a spawned tokio task; the draw loop only ever reads state that
//! task results have already written via [`Msg`] — it never awaits a
//! request itself (tui-design skill: "async everything").

use crate::api;
use crate::client::Client;
use anyhow::{Result, anyhow};
use raptor_api_types::{
    ActionRest, ActionStatusRest, DsRest, RolloutRest, SystemStatistics, TargetRest,
};
use ratatui::widgets::TableState;
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// A hung server must surface as an error, not an endless spinner with a new
/// stuck request piling up on every refresh tick.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub enum Msg {
    /// Tagged with the request generation: an auto-refresh still in flight
    /// when a new filter is applied must not overwrite the filtered result.
    Targets(u64, Result<Vec<TargetRest>>),
    Detail(String, Result<(Vec<ActionRest>, Vec<ActionStatusRest>)>),
    Rollouts(Result<Vec<RolloutRest>>),
    Stats(Result<SystemStatistics>),
    DsList(String, Result<Vec<DsRest>>),
    Done(Result<String>),
}

/// Modes that act on a target carry its controller ID, captured when the
/// mode opened: a refresh while the prompt is up can drop or reorder rows, and
/// the action must still land on the target the operator chose.
pub enum Mode {
    Normal,
    Search {
        input: String,
    },
    Assign {
        cid: String,
        filter: String,
        items: Vec<DsRest>,
        selected: usize,
    },
    TagInput {
        cid: String,
        input: String,
    },
    ConfirmCancel {
        cid: String,
        aid: i64,
    },
    ConfirmForce {
        cid: String,
        aid: i64,
    },
    Help,
}

pub struct App {
    pub client: Arc<Client>,
    pub tx: UnboundedSender<Msg>,

    pub targets: Vec<TargetRest>,
    /// Kept across frames: it holds the scroll offset as well as the selection.
    pub table: TableState,
    pub query: Option<String>,
    targets_gen: u64,

    pub detail_actions: Vec<ActionRest>,
    pub detail_history: Vec<ActionStatusRest>,
    pub rollouts: Vec<RolloutRest>,
    pub stats: Option<SystemStatistics>,

    pub mode: Mode,
    pub status: Option<(String, Instant)>,
    /// Latest failure per fetch, until that fetch next succeeds — a transient
    /// status line would expire and leave stale data looking current.
    pub errors: BTreeMap<&'static str, String>,
    pub loading: bool,
    pub should_quit: bool,

    pub refresh: Duration,
    pub last_refresh: Instant,
}

impl App {
    pub fn new(client: Client, refresh_secs: u64) -> (Self, UnboundedReceiver<Msg>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let app = Self {
            client: Arc::new(client),
            tx,
            targets: Vec::new(),
            table: TableState::default(),
            query: None,
            targets_gen: 0,
            detail_actions: Vec::new(),
            detail_history: Vec::new(),
            rollouts: Vec::new(),
            stats: None,
            mode: Mode::Normal,
            status: None,
            errors: BTreeMap::new(),
            loading: false,
            should_quit: false,
            refresh: Duration::from_secs(refresh_secs),
            last_refresh: Instant::now() - Duration::from_secs(3600),
        };
        (app, rx)
    }

    pub fn selected_target(&self) -> Option<&TargetRest> {
        self.table.selected().and_then(|i| self.targets.get(i))
    }

    #[cfg(test)]
    pub fn targets_gen(&self) -> u64 {
        self.targets_gen
    }

    pub fn selected_cid(&self) -> Option<String> {
        self.selected_target().map(|t| t.controller_id.clone())
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), Instant::now()));
    }

    pub fn tick(&mut self) {
        if let Some((_, at)) = &self.status
            && at.elapsed() > Duration::from_secs(4)
        {
            self.status = None;
        }
        let due = self.refresh > Duration::ZERO && self.last_refresh.elapsed() >= self.refresh;
        // Skip while the last refresh is still out; the timeout bounds the wait.
        if due && !self.loading {
            self.refresh_all();
        }
    }

    pub fn refresh_all(&mut self) {
        self.last_refresh = Instant::now();
        self.fetch_targets();
        self.fetch_rollouts();
        self.fetch_stats();
        if let Some(cid) = self.selected_cid() {
            self.fetch_detail(cid);
        }
    }

    /// Runs `fut` off the loop under [`REQUEST_TIMEOUT`] and posts its result
    /// back through `wrap`. A send error means the UI is gone; nothing to do.
    pub fn spawn<T, F>(&self, fut: F, wrap: impl FnOnce(Result<T>) -> Msg + Send + 'static)
    where
        T: Send + 'static,
        F: Future<Output = Result<T>> + Send + 'static,
    {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let r = tokio::time::timeout(REQUEST_TIMEOUT, fut)
                .await
                .unwrap_or_else(|_| Err(anyhow!("timed out after {}s", REQUEST_TIMEOUT.as_secs())));
            let _ = tx.send(wrap(r));
        });
    }

    pub fn fetch_targets(&mut self) {
        self.loading = true;
        self.targets_gen += 1;
        let generation = self.targets_gen;
        let client = self.client.clone();
        let args = api::ListArgs {
            q: self.query.clone(),
            sort: None,
            limit: Some(200),
            offset: None,
        };
        self.spawn(
            async move { Ok(api::targets::list(&client, &args).await?.content) },
            move |r| Msg::Targets(generation, r),
        );
    }

    pub fn fetch_rollouts(&self) {
        let client = self.client.clone();
        self.spawn(
            async move { api::rollouts::list(&client).await },
            Msg::Rollouts,
        );
    }

    pub fn fetch_stats(&self) {
        let client = self.client.clone();
        self.spawn(
            async move { api::system::statistics(&client).await },
            Msg::Stats,
        );
    }

    pub fn fetch_detail(&self, cid: String) {
        let client = self.client.clone();
        let tag = cid.clone();
        self.spawn(
            async move {
                let actions =
                    api::actions::list_for_target(&client, &cid, &api::ListArgs::default())
                        .await?
                        .content;
                let history = if let Some(a) = actions.first() {
                    api::actions::status_history(&client, &cid, a.id).await?
                } else {
                    Vec::new()
                };
                Ok((actions, history))
            },
            move |r| Msg::Detail(tag, r),
        );
    }

    pub fn fetch_ds_list(&self, cid: String) {
        let client = self.client.clone();
        let args = api::ListArgs {
            limit: Some(200),
            ..Default::default()
        };
        self.spawn(
            async move { Ok(api::distribution_sets::list(&client, &args).await?.content) },
            move |r| Msg::DsList(cid, r),
        );
    }

    fn record<T>(&mut self, source: &'static str, r: Result<T>) -> Option<T> {
        match r {
            Ok(v) => {
                self.errors.remove(source);
                Some(v)
            }
            Err(e) => {
                self.errors.insert(source, e.to_string());
                None
            }
        }
    }

    pub fn handle_msg(&mut self, msg: Msg) {
        match msg {
            Msg::Targets(generation, _) if generation != self.targets_gen => {}
            Msg::Targets(_, r) => {
                self.loading = false;
                if let Some(rows) = self.record("targets", r) {
                    self.set_targets(rows);
                }
            }
            Msg::Detail(cid, r) => {
                if let Some((actions, history)) = self.record("detail", r)
                    && self.selected_target().map(|t| &t.controller_id) == Some(&cid)
                {
                    self.detail_actions = actions;
                    self.detail_history = history;
                }
            }
            Msg::Rollouts(r) => {
                if let Some(r) = self.record("rollouts", r) {
                    self.rollouts = r;
                }
            }
            Msg::Stats(r) => {
                if let Some(s) = self.record("stats", r) {
                    self.stats = Some(s);
                }
            }
            Msg::DsList(cid, Ok(items)) => {
                // The operator may have opened another prompt meanwhile.
                if matches!(self.mode, Mode::Normal) {
                    self.mode = Mode::Assign {
                        cid,
                        filter: String::new(),
                        items,
                        selected: 0,
                    };
                }
            }
            Msg::DsList(_, Err(e)) => self.set_status(format!("distribution sets: {e}")),
            Msg::Done(Ok(msg)) => {
                self.set_status(msg);
                self.refresh_all();
            }
            Msg::Done(Err(e)) => self.set_status(format!("error: {e}")),
        }
    }

    /// Keeps the selection on the same target across a refresh; if it is
    /// gone, stays on the same row rather than jumping to the top.
    fn set_targets(&mut self, rows: Vec<TargetRest>) {
        let before = self.selected_cid();
        let row = self.table.selected().unwrap_or(0);
        self.targets = rows;
        let row = before
            .as_ref()
            .and_then(|cid| self.targets.iter().position(|t| &t.controller_id == cid))
            .unwrap_or(row);
        self.select(row);
        if self.selected_cid() != before {
            self.on_selection_changed();
        }
    }

    fn select(&mut self, row: usize) {
        let row = (!self.targets.is_empty()).then(|| row.min(self.targets.len() - 1));
        self.table.select(row);
    }

    pub fn select_next(&mut self) {
        self.move_to(self.table.selected().map_or(0, |i| i + 1));
    }

    pub fn select_prev(&mut self) {
        self.move_to(self.table.selected().unwrap_or(0).saturating_sub(1));
    }

    pub fn select_first(&mut self) {
        self.move_to(0);
    }

    pub fn select_last(&mut self) {
        self.move_to(usize::MAX);
    }

    fn move_to(&mut self, row: usize) {
        let before = self.table.selected();
        self.select(row);
        if self.table.selected() != before {
            self.on_selection_changed();
        }
    }

    fn on_selection_changed(&mut self) {
        self.detail_actions.clear();
        self.detail_history.clear();
        if let Some(cid) = self.selected_cid() {
            self.fetch_detail(cid);
        }
    }
}
