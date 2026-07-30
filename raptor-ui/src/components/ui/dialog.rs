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
        "w-96 rounded-lg border border-border-soft bg-card p-6",
        class.as_deref().unwrap_or("")
    );

    rsx! {
        if open() {
            // The backdrop takes focus itself (tabindex -1 + autofocus) so the
            // Escape handler has somewhere to land: opening a dialog otherwise
            // leaves focus on the trigger outside it, and keydown never arrives.
            // Landing here also means Tab walks forwards into the panel.
            div {
                class: "fixed inset-0 z-40 flex items-center justify-center bg-black/60",
                tabindex: "-1",
                autofocus: true,
                onkeydown: move |e| {
                    if e.key() == Key::Escape {
                        open.set(false);
                    }
                },
                onclick: move |_| open.set(false),
                div {
                    class: "{merged_class}",
                    role: "dialog",
                    aria_modal: "true",
                    // Clicks on the panel must not bubble to the backdrop's
                    // close handler, or every interaction would dismiss it.
                    onclick: move |e| e.stop_propagation(),
                    {children}
                }
            }
        }
    }
}
