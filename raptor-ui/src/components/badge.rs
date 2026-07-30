use dioxus::prelude::*;

/// A state as a coloured word, never colour alone: the label carries the meaning
/// for anyone reading a screenshot pasted into a ticket or unable to separate red
/// from green, and the tint carries it at a glance.
#[component]
pub fn StatusBadge(status: String) -> Element {
    let (label, tone) = crate::logic::status_style(&status);
    let tint = crate::components::tone_badge(tone);
    rsx! {
        span { class: "inline-block rounded border px-2 py-0.5 text-xs {tint}", "{label}" }
    }
}
