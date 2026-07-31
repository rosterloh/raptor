use crate::logic::Tone;
use dioxus::prelude::*;

/// A toggle filter. Chips rather than a `<select>` for the axes an operator
/// filters on constantly — the options and the current selection are both
/// visible without opening anything, and one click clears back to "all".
///
/// `aria-pressed` rather than a checkbox role: each chip is an independent
/// toggle, and a screen reader should hear its on/off state.
#[component]
pub fn Chip(
    label: String,
    /// Semantic dot, for chips that select a status.
    #[props(optional)]
    tone: Option<Tone>,
    /// Literal colour for the dot, already validated as a hex value by
    /// [`crate::logic::tag_colour`] — used for tag chips, whose colour is
    /// operator-supplied.
    #[props(optional)]
    dot: Option<String>,
    pressed: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let state = if pressed {
        "border-border bg-accent text-foreground"
    } else {
        "border-border-soft text-muted-foreground hover:border-border hover:text-fg-dim"
    };
    rsx! {
        button {
            r#type: "button",
            aria_pressed: pressed,
            class: "inline-flex items-center gap-1.5 rounded border px-3 py-1 font-mono text-[11px] tracking-[0.07em] uppercase transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none {state}",
            onclick: move |e| onclick.call(e),
            if let Some(t) = tone {
                span { class: "size-[7px] shrink-0 rounded-full {crate::components::tone_fill(t)}" }
            } else if let Some(c) = dot.clone() {
                span {
                    class: "size-[7px] shrink-0 rounded-full",
                    style: "background: {c}",
                }
            }
            "{label}"
        }
    }
}
