use crate::logic;
use dioxus::prelude::*;
use raptor_api_types::RolloutTargetsPerStatus;

/// Stacked per-status bar for a rollout or one of its groups. `total` is the
/// rollout/group target count, so a bar that does not fill leaves the remainder
/// as track — targets the server has not accounted for yet.
#[component]
pub fn ProgressBar(counts: RolloutTargetsPerStatus, total: i64) -> Element {
    rsx! {
        div { class: "flex h-2 w-full overflow-hidden rounded bg-accent",
            for (label , tone , n) in logic::progress_segments(&counts) {
                div {
                    key: "{label}",
                    class: "h-full {crate::components::tone_fill(tone)}",
                    style: "width: {logic::percent(n, total)}%",
                    title: "{n} {label}",
                }
            }
        }
    }
}

/// Colour-keyed counts under a [`ProgressBar`]: "3 finished · 1 running".
#[component]
pub fn ProgressLegend(counts: RolloutTargetsPerStatus) -> Element {
    rsx! {
        div { class: "flex flex-wrap gap-x-3 gap-y-1 text-xs text-fg-dim",
            for (label , tone , n) in logic::progress_segments(&counts) {
                span { key: "{label}", class: "flex items-center gap-1",
                    span { class: "inline-block h-2 w-2 rounded-sm {crate::components::tone_fill(tone)}" }
                    "{n} {label}"
                }
            }
        }
    }
}
