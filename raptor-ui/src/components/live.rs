//! Live updates from `GET /rest/v1/events` (SSE). Pages react to [`LiveEvent`]s
//! by refetching; while connected, `use_polling_every` slows to a safety-net beat.
// Task 8 wires the pages; until then some items are unused on the host build.
#![allow(dead_code)]

use std::cell::RefCell;
use std::hash::Hash;
use std::rc::Rc;

use dioxus::prelude::*;
use raptor_api_types::{ActionEvent, DownloadEvent, ProgressEvent, RolloutEvent, TargetEvent};

use crate::logic::Dirty;

#[derive(Debug, Clone, PartialEq)]
pub enum LiveEvent {
    Target(TargetEvent),
    Action(ActionEvent),
    Rollout(RolloutEvent),
    Download(DownloadEvent),
    Progress(ProgressEvent),
    /// Events may have been missed (lag or reconnect): refetch everything shown.
    Resync,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LiveFilter {
    pub target: Option<String>,
    pub rollout: Option<i64>,
}

/// Whether the live stream is currently up; provided in `Shell`.
#[derive(Clone, Copy)]
pub struct LiveContext(pub Signal<bool>);

impl LiveContext {
    pub fn provide() {
        use_context_provider(|| LiveContext(Signal::new(false)));
    }
}

#[cfg(target_arch = "wasm32")]
fn parse_event(name: &str, data: &str) -> Option<LiveEvent> {
    use raptor_api_types::*;
    Some(match name {
        EVENT_TARGET => LiveEvent::Target(serde_json::from_str(data).ok()?),
        EVENT_ACTION => LiveEvent::Action(serde_json::from_str(data).ok()?),
        EVENT_ROLLOUT => LiveEvent::Rollout(serde_json::from_str(data).ok()?),
        EVENT_DOWNLOAD => LiveEvent::Download(serde_json::from_str(data).ok()?),
        EVENT_PROGRESS => LiveEvent::Progress(serde_json::from_str(data).ok()?),
        EVENT_RESYNC => LiveEvent::Resync,
        _ => return None,
    })
}

/// An open `EventSource` plus the closures its listeners call; dropping closes
/// the source first so no callback can fire into a freed closure.
#[cfg(target_arch = "wasm32")]
struct Source {
    es: web_sys::EventSource,
    _closures: Vec<wasm_bindgen::closure::Closure<dyn FnMut(web_sys::Event)>>,
}

#[cfg(target_arch = "wasm32")]
impl Drop for Source {
    fn drop(&mut self) {
        self.es.close();
    }
}

#[cfg(target_arch = "wasm32")]
fn open_source(
    filter: &LiveFilter,
    on_event: Callback<LiveEvent>,
    mut connected: Signal<bool>,
    mut ctx: Option<LiveContext>,
    opened_before: Rc<std::cell::Cell<bool>>,
) -> Option<Source> {
    use raptor_api_types::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let mut q = Vec::new();
    if let Some(t) = &filter.target {
        q.push(format!("target={}", crate::logic::urlencode(t)));
    }
    if let Some(r) = filter.rollout {
        q.push(format!("rollout={r}"));
    }
    let url = format!(
        "{}/rest/v1/events{}{}",
        crate::api::base(),
        if q.is_empty() { "" } else { "?" },
        q.join("&")
    );
    let es = web_sys::EventSource::new(&url).ok()?;
    let mut closures: Vec<Closure<dyn FnMut(web_sys::Event)>> = Vec::new();
    let mut set = move |v: bool| {
        if *connected.peek() != v {
            connected.set(v);
        }
        if let Some(c) = ctx.as_mut()
            && *c.0.peek() != v
        {
            c.0.set(v);
        }
    };

    let mut listen = |name: &'static str, cb: Box<dyn FnMut(web_sys::Event)>| {
        let c = Closure::<dyn FnMut(web_sys::Event)>::wrap(cb);
        let _ = es.add_event_listener_with_callback(name, c.as_ref().unchecked_ref());
        closures.push(c);
    };
    for name in [
        EVENT_TARGET,
        EVENT_ACTION,
        EVENT_ROLLOUT,
        EVENT_DOWNLOAD,
        EVENT_PROGRESS,
        EVENT_RESYNC,
    ] {
        listen(
            name,
            Box::new(move |e: web_sys::Event| {
                let data = e
                    .dyn_ref::<web_sys::MessageEvent>()
                    .and_then(|m| m.data().as_string())
                    .unwrap_or_default();
                if let Some(ev) = parse_event(name, &data) {
                    on_event.call(ev);
                }
            }),
        );
    }
    let mut set_open = set;
    listen(
        "open",
        Box::new(move |_| {
            set_open(true);
            // Anything published while we were away is lost: tell the page.
            if opened_before.replace(true) {
                on_event.call(LiveEvent::Resync);
            }
        }),
    );
    let es2 = es.clone();
    listen(
        "error",
        Box::new(move |_| {
            if es2.ready_state() == web_sys::EventSource::CLOSED {
                set(false);
            }
        }),
    );
    Some(Source {
        es,
        _closures: closures,
    })
}

/// Subscribes to the event stream for as long as the component is mounted,
/// re-opening when `filter` changes. Returns whether the stream is connected.
/// Off-wasm this is inert and always false.
/// `on_event` is captured once at mount: capture only `Copy` handles (signals, resources), never plain values that change across renders.
pub fn use_live_events(filter: LiveFilter, on_event: Callback<LiveEvent>) -> Signal<bool> {
    let connected = use_signal(|| false);
    #[cfg(target_arch = "wasm32")]
    {
        let ctx = try_use_context::<LiveContext>();
        let slot: Rc<RefCell<Option<Source>>> = use_hook(Default::default);
        let opened_before: Rc<std::cell::Cell<bool>> = use_hook(Default::default);
        let s = slot.clone();
        use_effect(use_reactive!(|(filter,)| {
            // Drop (and close) the previous source before opening the next.
            s.borrow_mut().take();
            *s.borrow_mut() = open_source(&filter, on_event, connected, ctx, opened_before.clone());
        }));
        use_drop(move || {
            slot.borrow_mut().take();
            let (mut connected, mut ctx) = (connected, ctx);
            // Both signals may already be gone if the whole tree is unmounting.
            if let Some(c) = ctx.as_mut()
                && let Ok(mut w) = c.0.try_write()
            {
                *w = false;
            }
            if let Ok(mut w) = connected.try_write() {
                *w = false;
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (filter, on_event);
    connected
}

/// Returns a callback that marks `K` as changed; `on_due` runs for each marked
/// key at most once per second (checked on a 250 ms tick).
/// `on_due` is captured once at mount: capture only `Copy` handles (signals, resources), never plain values that change across renders.
pub fn use_coalesced_refetch<K: Eq + Hash + Clone + 'static>(
    on_due: impl FnMut(K) + 'static,
) -> Callback<K> {
    let dirty = use_hook(|| Rc::new(RefCell::new(Dirty::<K>::default())));
    let on_due = use_hook(|| Rc::new(RefCell::new(on_due)));
    let d = dirty.clone();
    use_future(move || {
        let (d, on_due) = (d.clone(), on_due.clone());
        async move {
            loop {
                gloo_timers::future::TimeoutFuture::new(250).await;
                let due = d.borrow_mut().take_due(super::now_ms());
                for k in due {
                    (on_due.borrow_mut())(k);
                }
            }
        }
    });
    use_callback(move |k| dirty.borrow_mut().mark(k))
}
