use dioxus::prelude::*;

/// A labelled hairline separating one region of a page from the next.
///
/// Cheaper than wrapping every group in its own card: the page stays one dense
/// surface, which is what a console full of tables wants, while still being
/// scannable. Deliberately quiet — the label is the smallest type on the screen.
#[component]
pub fn SectionRule(label: String) -> Element {
    rsx! {
        div { class: "mt-6 mb-3 flex items-center gap-3",
            span { class: "font-mono text-[11px] font-medium tracking-[0.1em] text-muted-foreground uppercase",
                "{label}"
            }
            span { class: "h-px flex-1 bg-border-soft" }
        }
    }
}
