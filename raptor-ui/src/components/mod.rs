//! Shared UI components and Tailwind class constants used across pages.
//! `ui` holds the vendored shadcn-style components (see its own doc); everything
//! else here is hand-rolled for this app. Both draw their colours from the
//! semantic tokens in `tailwind.css` rather than naming palette shades, so the
//! two layers stay one design system.

pub mod badge;
pub mod chip;
pub mod command_palette;
pub mod confirm;
pub mod error_pane;
pub mod paginator;
pub mod progress;
pub mod search;
pub mod section;
pub mod tabs;
pub mod tag_chip;
pub mod toast;
pub mod ui;

pub use badge::StatusBadge;
pub use chip::Chip;
pub use command_palette::CommandPalette;
pub use confirm::ConfirmDialog;
pub use error_pane::ErrorPane;
pub use paginator::Paginator;
pub use progress::{ProgressBar, ProgressLegend};
pub use search::SearchBox;
pub use section::SectionRule;
pub use tabs::{TabPanel, Tabs};
pub use tag_chip::TagChip;
pub use toast::{ToastStack, toast_error, toast_ok};

use dioxus::prelude::*;

use crate::logic::Tone;

/// Badge classes for a [`Tone`] — tinted surface, readable label, visible edge.
/// This is the only place a tone becomes colour, so `logic.rs` stays free of
/// presentation and a palette change stays inside `tailwind.css`.
pub fn tone_badge(tone: Tone) -> &'static str {
    match tone {
        Tone::Ok => "bg-ok-bg text-ok-fg border-ok-border",
        Tone::Pending => "bg-pend-bg text-pend-fg border-pend-border",
        Tone::Error => "bg-err-bg text-err-fg border-err-border",
        Tone::Info => "bg-info-bg text-info-fg border-info-border",
        Tone::Neutral => "bg-neutral-bg text-neutral-fg border-neutral-border",
    }
}

/// Solid fill for a [`Tone`] — progress-bar segments and status dots, where the
/// colour is the whole encoding and needs full saturation.
pub fn tone_fill(tone: Tone) -> &'static str {
    match tone {
        Tone::Ok => "bg-ok",
        Tone::Pending => "bg-pend",
        Tone::Error => "bg-err",
        Tone::Info => "bg-info",
        Tone::Neutral => "bg-neutral",
    }
}

/// Text colour for a [`Tone`] — a figure that *is* its status, like a dashboard
/// counter.
///
/// These must stay literal strings. Tailwind scans source text for class names,
/// so a runtime-built `"text-{tone}"` produces no rule at all and the element
/// renders unstyled; returning the literal from a match is what makes it work.
pub fn tone_text(tone: Tone) -> &'static str {
    match tone {
        Tone::Ok => "text-ok",
        Tone::Pending => "text-pend",
        Tone::Error => "text-err",
        Tone::Info => "text-info",
        Tone::Neutral => "text-neutral",
    }
}

/// A `<select>` matching [`ui::Input`]'s border, height and focus ring — the two
/// sit side by side in the create dialogs. Selects can't reuse the `Input`
/// component itself (that renders an `<input>`), so the class list lives here.
pub const SELECT: &str = "mb-3 flex h-9 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-1 text-base shadow-xs outline-none transition-[color,box-shadow] focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-2";
pub const HEADING: &str = "mb-4 text-xl font-bold text-foreground";
pub const TABLE: &str = "w-full border-collapse text-sm";
pub const TH: &str =
    "border-b border-border-soft px-3 py-2 text-left font-medium text-muted-foreground";
pub const TD: &str = "border-b border-border-subtle px-3 py-2";
/// Table row: a hover affordance, not a click target. A `<tr>` can't take
/// keyboard focus, so rows don't handle clicks — the first cell carries a real
/// `Link` ([`LINK_CELL`]) that keyboard nav, middle-click and "copy link
/// address" all understand.
pub const ROW: &str = "hover:bg-card";
/// The `Link` filling a list table's first cell, standing in for a row click.
/// `block` stretches it across the cell so the whole width stays clickable.
pub const LINK_CELL: &str = "block text-primary hover:underline";

/// Restart a resource every 5s while mounted (dashboard, running actions).
pub fn use_polling<T: 'static>(res: Resource<T>) {
    use_polling_every(res, 5_000);
}

/// [`use_polling`] with an explicit interval, for data that moves slowly enough
/// that the 5s tick would just be waste. Saved target filters and the sets they
/// auto-assign are edited by hand, not by devices, so the dashboard's segment
/// panel refreshes on a much longer beat than its live counters — otherwise each
/// segment would multiply the per-tick request count.
///
/// Skips the restart while the tab is hidden either way. Without the gate, every
/// console left open in a background tab keeps polling the API indefinitely — on
/// the dashboard that is the most expensive read the app makes. Data is up to one
/// interval stale when the tab comes back, which the next tick clears.
pub fn use_polling_every<T: 'static>(mut res: Resource<T>, ms: u32) {
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(ms).await;
            if !tab_hidden() {
                res.restart();
            }
        }
    });
}

/// Wall-clock milliseconds, for turning a stored timestamp into an age. Lives
/// here beside [`tab_hidden`] rather than in `logic`, which stays free of
/// platform lookups so it can be tested on the host; `logic::relative_age` takes
/// `now` as an argument for the same reason. Returns 0 off-wasm.
pub fn now_ms() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

/// The command palette's "Clear filter" action needs to reach into whichever
/// list page is currently mounted without the two knowing about each other
/// directly. A page registers its own active-filter check and clear behaviour
/// via [`use_filter_clear`]; the palette just reads this context.
#[derive(Clone, Copy)]
pub struct FilterClear {
    active: Signal<bool>,
    clear: Signal<Option<Callback<()>>>,
}

impl FilterClear {
    /// Call once, in `Shell`, to seed the context every page and the palette
    /// share.
    pub fn provide() {
        use_context_provider(|| FilterClear {
            active: Signal::new(false),
            clear: Signal::new(None),
        });
    }

    pub(crate) fn active(&self) -> bool {
        (self.active)()
    }

    pub(crate) fn clear(&self) {
        if let Some(cb) = (self.clear)() {
            cb.call(());
        }
    }
}

/// Registers this page's filter state with the command palette. `is_active` is
/// evaluated inside the tracked effect, so pass a closure reading the page's
/// own filter signals (not a plain `bool`) or the palette's "Clear filter" item
/// won't react to later changes. Deregisters on unmount so navigating away
/// doesn't leave the palette pointed at an unmounted page.
pub fn use_filter_clear(
    mut is_active: impl FnMut() -> bool + 'static,
    on_clear: impl FnMut() + 'static,
) {
    let mut ctx = use_context::<FilterClear>();
    // `on_clear` mutates signals, so it needs `FnMut`; wrapped so the `Callback`
    // handed to the palette can still be called through a shared reference.
    let on_clear = std::rc::Rc::new(std::cell::RefCell::new(on_clear));
    use_effect(move || {
        ctx.active.set(is_active());
        let on_clear = on_clear.clone();
        ctx.clear
            .set(Some(Callback::new(move |()| (on_clear.borrow_mut())())));
    });
    use_drop(move || {
        ctx.active.set(false);
        ctx.clear.set(None);
    });
}

/// Whether the document is currently hidden — a backgrounded tab or a minimised
/// window. Always false off-wasm, where there is no document to ask.
fn tab_hidden() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.document())
            .is_some_and(|d| d.hidden())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}
