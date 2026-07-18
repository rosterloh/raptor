// Adapted from https://github.com/rust-ui/dioxus-ui's dialog.rs concept, but not
// vendored verbatim: upstream drives open/close with a <script>-injected vanilla-JS
// DOM patch and an undefined `window.ScrollLock` global, which doesn't fit this
// codebase's Dioxus-signal-driven components (see ConfirmDialog/CreateModuleDialog).
// This rewrite keeps the same visual language but is driven entirely by a
// `Signal<bool>`, matching the reactive pattern already used elsewhere.

use dioxus::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn Dialog(
    open: Signal<bool>,
    #[props(into, optional)] class: Option<String>,
    children: Element,
) -> Element {
    let merged_class = tw_merge!(
        "w-96 rounded-lg border border-zinc-800 bg-zinc-900 p-6",
        class.as_deref().unwrap_or("")
    );

    rsx! {
        if open() {
            div { class: "fixed inset-0 z-40 flex items-center justify-center bg-black/60",
                div { class: "{merged_class}", {children} }
            }
        }
    }
}
