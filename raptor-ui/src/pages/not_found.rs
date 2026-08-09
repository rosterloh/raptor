use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn NotFound(route: Vec<String>) -> Element {
    let path = format!("/{}", route.join("/"));
    rsx! {
        div { class: "flex flex-col items-start gap-3",
            h1 { class: "font-display text-3xl font-bold tracking-wider uppercase text-foreground",
                "Not found"
            }
            p { class: "font-mono text-xs text-muted-foreground", "No route matches {path}" }
            Link {
                to: Route::Dashboard {},
                class: "text-sm text-fg-dim underline underline-offset-4 hover:text-foreground",
                "Back to dashboard →"
            }
        }
    }
}
