use crate::Route;
use crate::components::{CommandPalette, FilterClear, ToastStack};
use dioxus::prelude::*;

const LOGO: Asset = asset!("/assets/logo/logo-sidebar.png");

#[component]
pub fn Shell() -> Element {
    let nav = use_navigator();
    FilterClear::provide();
    let mut palette_open = use_signal(|| false);
    // Confirm the session before painting a logged-in shell. Previously an
    // unauthenticated visit to /ui/targets rendered the whole sidebar and a
    // "Loading…", then jumped to the login page once the first API call came
    // back 401. The probe runs once per mount, not per navigation, because the
    // router keeps the layout mounted across child routes.
    let session = use_resource(|| async { crate::api::session().await });
    if !matches!(&*session.read_unchecked(), Some(Ok(()))) {
        // In flight, or refused — and a refusal has already navigated to the
        // login page from the api layer's 401 handling. Nothing to draw either way.
        return rsx! {};
    }
    rsx! {
        div {
            class: "flex min-h-screen bg-background text-foreground",
            // Keydown bubbles here from any focused descendant (nav links, search
            // boxes, dialogs), so ⌘K/Ctrl-K opens the palette no matter what has
            // focus — a real global shortcut would need a window-level listener,
            // which this sidesteps.
            onkeydown: move |e: KeyboardEvent| {
                let combo = e.modifiers().meta() || e.modifiers().ctrl();
                if combo && e.key() == Key::Character("k".to_string()) {
                    e.prevent_default();
                    palette_open.set(true);
                }
            },
            aside { class: "flex w-52 flex-col border-r border-border-soft bg-card",
                div { class: "flex items-center gap-2 px-4 py-5",
                    img { src: LOGO, class: "h-8 w-8", alt: "" }
                    span { class: "text-lg font-bold tracking-wide text-primary", "raptor" }
                }
                nav { class: "flex flex-1 flex-col gap-1 px-2",
                    NavLink { to: Route::Dashboard {}, label: "Dashboard" }
                    NavLink { to: Route::Targets {}, label: "Targets" }
                    NavLink { to: Route::TargetFilters {}, label: "Target filters" }
                    NavLink { to: Route::Distributions {}, label: "Distributions" }
                    NavLink { to: Route::Modules {}, label: "Modules" }
                    NavLink { to: Route::Rollouts {}, label: "Rollouts" }
                    NavLink { to: Route::Actions {}, label: "Actions" }
                    NavLink { to: Route::Tags {}, label: "Tags" }
                }
                button {
                    class: "mx-2 mb-1 flex items-center justify-between rounded px-3 py-2 text-left text-sm text-fg-dim hover:bg-accent",
                    onclick: move |_| palette_open.set(true),
                    span { "Jump to…" }
                    kbd { class: "rounded border border-border-soft px-1.5 py-0.5 font-mono text-xs text-muted-foreground",
                        "⌘K"
                    }
                }
                button {
                    class: "m-2 rounded px-3 py-2 text-left text-sm text-fg-dim hover:bg-accent",
                    onclick: move |_| async move {
                        let _ = crate::api::logout().await;
                        nav.push(Route::Login {});
                    },
                    "Log out"
                }
            }
            main { class: "flex-1 overflow-x-auto p-6",
                Outlet::<Route> {}
                ToastStack {}
            }
        }
        CommandPalette { open: palette_open }
    }
}

#[component]
fn NavLink(to: Route, label: String) -> Element {
    rsx! {
        Link {
            to,
            class: "rounded px-3 py-2 text-sm text-fg-dim hover:bg-accent",
            // The current page is marked by an accent edge plus full-strength
            // text, not by accent-coloured text: the accent is capped at about
            // two visible uses per screen and the brand mark already spends one.
            active_class: "bg-accent text-foreground shadow-[inset_2px_0_0_var(--color-primary)]",
            "{label}"
        }
    }
}
