//! Shared UI components and Tailwind class constants used across pages.
//! `ui` holds the vendored shadcn-style components (see its own doc); everything
//! else here is hand-rolled for this app. Both draw their colours from the
//! semantic tokens in `tailwind.css` rather than naming palette shades, so the
//! two layers stay one design system.

pub mod badge;
pub mod confirm;
pub mod error_pane;
pub mod paginator;
pub mod progress;
pub mod search;
pub mod tag_chip;
pub mod toast;
pub mod ui;

pub use badge::StatusBadge;
pub use confirm::ConfirmDialog;
pub use error_pane::ErrorPane;
pub use paginator::Paginator;
pub use progress::{ProgressBar, ProgressLegend};
pub use search::SearchBox;
pub use tag_chip::TagChip;
pub use toast::{toast_error, toast_ok, ToastStack};

use dioxus::prelude::*;

/// A `<select>` matching [`ui::Input`]'s border, height and focus ring — the two
/// sit side by side in the create dialogs. Selects can't reuse the `Input`
/// component itself (that renders an `<input>`), so the class list lives here.
pub const SELECT: &str = "mb-3 flex h-9 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-1 text-base shadow-xs outline-none transition-[color,box-shadow] focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-2";
pub const HEADING: &str = "mb-4 text-xl font-bold text-foreground";
pub const TABLE: &str = "w-full border-collapse text-sm";
pub const TH: &str =
    "border-b border-border-soft px-3 py-2 text-left font-medium text-muted-foreground";
pub const TD: &str = "border-b border-border-subtle px-3 py-2";
pub const ROW: &str = "cursor-pointer hover:bg-card";

/// Restart a resource every 5s while mounted (dashboard, running actions).
pub fn use_polling<T: 'static>(mut res: Resource<T>) {
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5_000).await;
            res.restart();
        }
    });
}
