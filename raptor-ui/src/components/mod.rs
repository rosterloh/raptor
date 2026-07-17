pub mod badge;
pub mod confirm;
pub mod error_pane;
pub mod paginator;
pub mod search;
pub mod toast;
pub mod ui;

pub use badge::StatusBadge;
pub use confirm::ConfirmDialog;
pub use error_pane::ErrorPane;
pub use paginator::Paginator;
pub use search::SearchBox;
pub use toast::{toast_error, toast_ok, ToastStack};

use dioxus::prelude::*;

pub const INPUT: &str = "mb-3 w-full rounded border border-zinc-700 bg-zinc-950 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:border-emerald-600 focus:outline-none";
pub const BTN: &str = "rounded bg-emerald-700 px-3 py-1.5 text-sm font-medium text-white hover:bg-emerald-600 disabled:opacity-50";
pub const BTN_DANGER: &str =
    "rounded bg-red-800 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-700";
pub const HEADING: &str = "mb-4 text-xl font-bold text-zinc-100";
pub const TABLE: &str = "w-full border-collapse text-sm";
pub const TH: &str = "border-b border-zinc-800 px-3 py-2 text-left font-medium text-zinc-500";
pub const TD: &str = "border-b border-zinc-900 px-3 py-2";
pub const ROW: &str = "cursor-pointer hover:bg-zinc-900";
pub const CARD: &str = "rounded-lg border border-zinc-800 bg-zinc-900 p-4";

/// Restart a resource every 5s while mounted (dashboard, running actions).
pub fn use_polling<T: 'static>(mut res: Resource<T>) {
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5_000).await;
            res.restart();
        }
    });
}
