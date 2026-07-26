use dioxus::prelude::*;

/// A tag as a small chip: name plus a colour dot when the tag carries a usable
/// colour (see `logic::tag_colour` — anything else renders without the dot).
#[component]
pub fn TagChip(name: String, colour: Option<String>) -> Element {
    let dot = crate::logic::tag_colour(colour.as_deref());
    rsx! {
        span { class: "inline-flex items-center gap-1.5 rounded border border-zinc-700 bg-zinc-800 px-2 py-0.5 text-xs text-zinc-200",
            if let Some(c) = dot {
                span {
                    class: "inline-block h-2 w-2 shrink-0 rounded-full",
                    style: "background-color: {c}",
                }
            }
            "{name}"
        }
    }
}
