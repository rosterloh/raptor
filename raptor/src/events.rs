//! In-process event hub: a broadcast channel for live updates plus a small
//! table of the latest download/progress state per action, so a new SSE
//! subscriber can be primed. Nothing here is persisted.

use raptor_api_types::{
    ActionEvent, DownloadEvent, EVENT_ACTION, EVENT_DOWNLOAD, EVENT_PROGRESS, EVENT_ROLLOUT,
    EVENT_TARGET, ProgressEvent, RolloutEvent, TargetEvent,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, watch};

const DOWNLOAD_THROTTLE: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
pub enum Event {
    Target(TargetEvent),
    Action(ActionEvent),
    Rollout(RolloutEvent),
    Download(DownloadEvent),
    Progress(ProgressEvent),
}

impl Event {
    /// SSE `event:` name.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Target(_) => EVENT_TARGET,
            Event::Action(_) => EVENT_ACTION,
            Event::Rollout(_) => EVENT_ROLLOUT,
            Event::Download(_) => EVENT_DOWNLOAD,
            Event::Progress(_) => EVENT_PROGRESS,
        }
    }

    pub fn controller_id(&self) -> Option<&str> {
        match self {
            Event::Target(e) => Some(&e.controller_id),
            Event::Action(e) => Some(&e.controller_id),
            Event::Rollout(_) => None,
            Event::Download(e) => Some(&e.controller_id),
            Event::Progress(e) => Some(&e.controller_id),
        }
    }

    pub fn rollout_id(&self) -> Option<i64> {
        match self {
            Event::Rollout(e) => Some(e.rollout_id),
            Event::Action(e) => e.rollout_id,
            _ => None,
        }
    }

    /// High-frequency events delivered only to per-target subscriptions.
    pub fn is_data(&self) -> bool {
        matches!(self, Event::Download(_) | Event::Progress(_))
    }
}

#[derive(Default)]
struct Table {
    /// Latest download per `(action_id, filename)` and when it last published.
    downloads: HashMap<(i64, String), (DownloadEvent, Instant)>,
    progress: HashMap<i64, ProgressEvent>,
}

pub struct Events {
    tx: broadcast::Sender<Event>,
    table: Mutex<Table>,
    shutdown: watch::Sender<bool>,
}

impl Default for Events {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl Events {
    pub fn new(capacity: usize) -> Self {
        Self {
            tx: broadcast::channel(capacity).0,
            table: Mutex::default(),
            shutdown: watch::channel(false).0,
        }
    }

    pub fn publish(&self, ev: Event) {
        // Err only means no subscribers.
        let _ = self.tx.send(ev);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub fn record_download(&self, ev: DownloadEvent) {
        let now = Instant::now();
        let key = (ev.action_id, ev.filename.clone());
        let publish = {
            let mut t = self.table.lock().unwrap();
            let due = ev.sent == ev.total
                || t.downloads
                    .get(&key)
                    .is_none_or(|(_, last)| now.duration_since(*last) >= DOWNLOAD_THROTTLE);
            let last = if due { now } else { t.downloads[&key].1 };
            t.downloads.insert(key, (ev.clone(), last));
            due
        };
        if publish {
            self.publish(Event::Download(ev));
        }
    }

    pub fn record_progress(&self, ev: ProgressEvent) {
        self.table
            .lock()
            .unwrap()
            .progress
            .insert(ev.action_id, ev.clone());
        self.publish(Event::Progress(ev));
    }

    pub fn clear_action(&self, action_id: i64) {
        let mut t = self.table.lock().unwrap();
        t.downloads.retain(|(id, _), _| *id != action_id);
        t.progress.remove(&action_id);
    }

    pub fn snapshot(&self, controller_id: &str) -> Vec<Event> {
        let t = self.table.lock().unwrap();
        let downloads = t
            .downloads
            .values()
            .filter(|(d, _)| d.controller_id == controller_id)
            .map(|(d, _)| Event::Download(d.clone()));
        let progress = t
            .progress
            .values()
            .filter(|p| p.controller_id == controller_id)
            .map(|p| Event::Progress(p.clone()));
        downloads.chain(progress).collect()
    }

    pub fn action_ids(&self) -> Vec<i64> {
        let t = self.table.lock().unwrap();
        let mut ids: Vec<i64> = t
            .downloads
            .keys()
            .map(|(id, _)| *id)
            .chain(t.progress.keys().copied())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub fn shutdown(&self) {
        self.shutdown.send_replace(true);
    }

    pub fn shutdown_signal(&self) -> watch::Receiver<bool> {
        self.shutdown.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dl(cid: &str, sent: u64, total: u64) -> DownloadEvent {
        DownloadEvent {
            controller_id: cid.into(),
            action_id: 1,
            filename: "f.bin".into(),
            sent,
            total,
        }
    }

    fn prog(cid: &str, action_id: i64) -> ProgressEvent {
        ProgressEvent {
            controller_id: cid.into(),
            action_id,
            cnt: 1,
            of: 2,
        }
    }

    #[test]
    fn publish_reaches_subscriber() {
        let ev = Events::new(8);
        let mut rx = ev.subscribe();
        ev.publish(Event::Target(TargetEvent {
            controller_id: "dev-1".into(),
        }));
        assert_eq!(rx.try_recv().unwrap().controller_id(), Some("dev-1"));
    }

    #[test]
    fn download_is_throttled_but_final_chunk_always_publishes() {
        let ev = Events::new(8);
        let mut rx = ev.subscribe();
        ev.record_download(dl("dev-1", 1, 3));
        ev.record_download(dl("dev-1", 2, 3));
        ev.record_download(dl("dev-1", 3, 3));
        let sents: Vec<u64> = std::iter::from_fn(|| rx.try_recv().ok())
            .map(|e| match e {
                Event::Download(d) => d.sent,
                _ => panic!("unexpected event"),
            })
            .collect();
        assert_eq!(sents, vec![1, 3]);
        match ev.snapshot("dev-1").as_slice() {
            [Event::Download(d)] => assert_eq!(d.sent, 3),
            other => panic!("unexpected snapshot {other:?}"),
        }
    }

    #[test]
    fn clear_action_empties_snapshot() {
        let ev = Events::new(8);
        ev.record_download(dl("dev-1", 1, 3));
        ev.record_progress(prog("dev-1", 1));
        assert_eq!(ev.snapshot("dev-1").len(), 2);
        assert_eq!(ev.action_ids(), vec![1]);
        ev.clear_action(1);
        assert!(ev.snapshot("dev-1").is_empty());
        assert!(ev.action_ids().is_empty());
    }

    #[test]
    fn snapshot_is_scoped_to_target() {
        let ev = Events::new(8);
        ev.record_progress(prog("dev-1", 1));
        ev.record_progress(prog("dev-2", 2));
        let snap = ev.snapshot("dev-1");
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].controller_id(), Some("dev-1"));
    }

    #[test]
    fn shutdown_flips_signal() {
        let ev = Events::new(8);
        let rx = ev.shutdown_signal();
        assert!(!*rx.borrow());
        ev.shutdown();
        assert!(*rx.borrow());
    }
}
