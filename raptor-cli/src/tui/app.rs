//! TUI application state and the async result channel. Every network call
//! runs on a spawned tokio task; the draw loop only ever reads state that
//! task results have already written via [`Msg`] — it never awaits a
//! request itself (tui-design skill: "async everything").

use crate::api;
use crate::client::Client;
use anyhow::Result;
use raptor_api_types::{
    ActionRest, ActionStatusRest, DsRest, RolloutRest, SystemStatistics, TargetRest,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub enum Msg {
    Targets(Result<Vec<TargetRest>>),
    Detail(String, Result<(Vec<ActionRest>, Vec<ActionStatusRest>)>),
    Rollouts(Result<Vec<RolloutRest>>),
    Stats(Result<SystemStatistics>),
    DsList(Result<Vec<DsRest>>),
    Done(Result<String>),
}

pub enum Mode {
    Normal,
    Search {
        input: String,
    },
    Assign {
        filter: String,
        items: Vec<DsRest>,
        selected: usize,
    },
    TagInput {
        input: String,
    },
    ConfirmCancel(i64),
    ConfirmForce(i64),
    Help,
}

pub struct App {
    pub client: Arc<Client>,
    pub tx: UnboundedSender<Msg>,

    pub targets: Vec<TargetRest>,
    pub selected: usize,
    pub query: Option<String>,

    pub detail_actions: Vec<ActionRest>,
    pub detail_history: Vec<ActionStatusRest>,
    pub rollouts: Vec<RolloutRest>,
    pub stats: Option<SystemStatistics>,

    pub mode: Mode,
    pub status: Option<(String, Instant)>,
    pub loading: bool,
    pub should_quit: bool,

    pub refresh: Duration,
    pub last_refresh: Instant,
    pub last_fetch_start: Option<Instant>,
}

impl App {
    pub fn new(client: Client, refresh_secs: u64) -> (Self, UnboundedReceiver<Msg>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let app = Self {
            client: Arc::new(client),
            tx,
            targets: Vec::new(),
            selected: 0,
            query: None,
            detail_actions: Vec::new(),
            detail_history: Vec::new(),
            rollouts: Vec::new(),
            stats: None,
            mode: Mode::Normal,
            status: None,
            loading: false,
            should_quit: false,
            refresh: Duration::from_secs(refresh_secs),
            last_refresh: Instant::now() - Duration::from_secs(3600),
            last_fetch_start: None,
        };
        (app, rx)
    }

    pub fn selected_target(&self) -> Option<&TargetRest> {
        self.targets.get(self.selected)
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
        if due {
            self.refresh_all();
        }
    }

    pub fn refresh_all(&mut self) {
        self.last_refresh = Instant::now();
        self.last_fetch_start = Some(Instant::now());
        self.loading = true;
        self.fetch_targets();
        self.fetch_rollouts();
        self.fetch_stats();
        if let Some(t) = self.selected_target() {
            self.fetch_detail(t.controller_id.clone());
        }
    }

    pub fn fetch_targets(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        let args = api::ListArgs {
            q: self.query.clone(),
            sort: None,
            limit: Some(200),
            offset: None,
        };
        tokio::spawn(async move {
            let r = api::targets::list(&client, &args).await.map(|p| p.content);
            let _ = tx.send(Msg::Targets(r));
        });
    }

    pub fn fetch_rollouts(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let r = api::rollouts::list(&client).await;
            let _ = tx.send(Msg::Rollouts(r));
        });
    }

    pub fn fetch_stats(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let r = api::system::statistics(&client).await;
            let _ = tx.send(Msg::Stats(r));
        });
    }

    pub fn fetch_detail(&self, cid: String) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let r = async {
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
            }
            .await;
            let _ = tx.send(Msg::Detail(cid, r));
        });
    }

    pub fn fetch_ds_list(&self) {
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let r = api::distribution_sets::list(
                &client,
                &api::ListArgs {
                    limit: Some(200),
                    ..Default::default()
                },
            )
            .await
            .map(|p| p.content);
            let _ = tx.send(Msg::DsList(r));
        });
    }

    pub fn handle_msg(&mut self, msg: Msg) {
        match msg {
            Msg::Targets(Ok(rows)) => {
                self.loading = false;
                let selected_cid = self.selected_target().map(|t| t.controller_id.clone());
                self.targets = rows;
                self.selected = selected_cid
                    .and_then(|cid| self.targets.iter().position(|t| t.controller_id == cid))
                    .unwrap_or(0)
                    .min(self.targets.len().saturating_sub(1));
            }
            Msg::Targets(Err(e)) => {
                self.loading = false;
                self.set_status(format!("targets: {e}"));
            }
            Msg::Detail(cid, Ok((actions, history))) => {
                if self.selected_target().map(|t| &t.controller_id) == Some(&cid) {
                    self.detail_actions = actions;
                    self.detail_history = history;
                }
            }
            Msg::Detail(_, Err(e)) => self.set_status(format!("detail: {e}")),
            Msg::Rollouts(Ok(r)) => self.rollouts = r,
            Msg::Rollouts(Err(e)) => self.set_status(format!("rollouts: {e}")),
            Msg::Stats(Ok(s)) => self.stats = Some(s),
            Msg::Stats(Err(e)) => self.set_status(format!("stats: {e}")),
            Msg::DsList(Ok(items)) => {
                self.mode = Mode::Assign {
                    filter: String::new(),
                    items,
                    selected: 0,
                };
            }
            Msg::DsList(Err(e)) => {
                self.mode = Mode::Normal;
                self.set_status(format!("distribution sets: {e}"));
            }
            Msg::Done(Ok(msg)) => {
                self.set_status(msg);
                self.refresh_all();
            }
            Msg::Done(Err(e)) => self.set_status(format!("error: {e}")),
        }
    }

    pub fn select_next(&mut self) {
        if !self.targets.is_empty() {
            self.selected = (self.selected + 1).min(self.targets.len() - 1);
            self.on_selection_changed();
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.on_selection_changed();
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
        self.on_selection_changed();
    }

    pub fn select_last(&mut self) {
        self.selected = self.targets.len().saturating_sub(1);
        self.on_selection_changed();
    }

    fn on_selection_changed(&mut self) {
        self.detail_actions.clear();
        self.detail_history.clear();
        if let Some(t) = self.selected_target() {
            self.fetch_detail(t.controller_id.clone());
        }
    }
}
