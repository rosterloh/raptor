use dioxus::prelude::*;

#[component]
pub fn StatusBadge(status: String) -> Element {
    let (label, classes) = crate::logic::status_style(&status);
    rsx! {
        span { class: "inline-block rounded border px-2 py-0.5 text-xs {classes}", "{label}" }
    }
}
