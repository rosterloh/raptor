// Adapted from shadcn's Card concept (a simple styled container), matching this
// codebase's existing CARD class constant rather than upstream's markup verbatim.

use dioxus::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn Card(#[props(into, optional)] class: Option<String>, children: Element) -> Element {
    let merged_class = tw_merge!(
        "rounded-lg border border-zinc-800 bg-zinc-900 p-4",
        class.as_deref().unwrap_or("")
    );

    rsx! {
        div { class: "{merged_class}", {children} }
    }
}
