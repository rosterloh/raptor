use crate::Route;
use crate::components::ui::Dialog;
use crate::components::{CommandPalette, FilterClear, ToastStack, use_theme};
use dioxus::prelude::*;

const LOGO: Asset = asset!("/assets/logo/logo-sidebar.png");

#[component]
pub fn Shell() -> Element {
    FilterClear::provide();
    let mut palette_open = use_signal(|| false);
    let mut drawer_open = use_signal(|| false);
    let (is_dark, toggle_theme) = use_theme();
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
            class: "flex min-h-screen flex-col bg-background text-foreground lg:flex-row",
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
            header { class: "sticky top-0 z-30 flex h-14 items-center justify-between border-b border-border-soft bg-card px-4 lg:hidden",
                div { class: "flex items-center gap-2",
                    img { src: LOGO, class: "h-8 w-8", alt: "" }
                    span { class: "font-display text-lg font-bold tracking-wide text-primary", "raptor" }
                }
                button {
                    class: "min-h-11 rounded border border-border px-4 text-sm text-foreground hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                    aria_label: "Open navigation",
                    onclick: move |_| drawer_open.set(true),
                    "Menu"
                }
            }
            aside { class: "hidden w-52 shrink-0 border-r border-border-soft bg-card lg:flex",
                Sidebar { palette_open, drawer_open: None, is_dark, toggle_theme }
            }
            main { class: "min-w-0 flex-1 overflow-x-auto p-4 sm:p-6",
                Outlet::<Route> {}
                ToastStack {}
            }
        }
        Dialog {
            open: drawer_open,
            aria_label: "Navigation".to_string(),
            backdrop_class: "items-stretch justify-start lg:hidden".to_string(),
            class: "h-full w-72 max-w-[85vw] rounded-none border-y-0 border-l-0 p-0 shadow-lg".to_string(),
            Sidebar { palette_open, drawer_open: Some(drawer_open), is_dark, toggle_theme }
        }
        CommandPalette { open: palette_open }
    }
}

#[component]
fn Sidebar(
    palette_open: Signal<bool>,
    drawer_open: Option<Signal<bool>>,
    is_dark: Signal<bool>,
    toggle_theme: Callback<()>,
) -> Element {
    let nav = use_navigator();
    let close = move |()| {
        if let Some(mut open) = drawer_open {
            open.set(false);
        }
    };
    rsx! {
        div { class: "flex h-full w-full flex-col",
            div { class: "flex items-center justify-between gap-2 px-4 py-5",
                div { class: "flex items-center gap-2",
                    img { src: LOGO, class: "h-8 w-8", alt: "" }
                    span { class: "font-display text-lg font-bold tracking-wide text-primary", "raptor" }
                }
                if drawer_open.is_some() {
                    button {
                        class: "min-h-11 rounded px-3 text-sm text-fg-dim hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                        aria_label: "Close navigation",
                        onclick: move |_| close(()),
                        "Close"
                    }
                }
            }
            nav { class: "flex flex-1 flex-col gap-4 overflow-y-auto px-2 pb-4",
                for (group , links) in nav_groups() {
                    div { key: "{group}",
                        p { class: "px-3 pb-1 font-mono text-[10px] tracking-[0.09em] text-muted-foreground uppercase", "{group}" }
                        div { class: "flex flex-col gap-1",
                            for (label , route) in links {
                                NavLink { key: "{label}", to: route, label, on_navigate: move |_| close(()) }
                            }
                        }
                    }
                }
            }
            button {
                class: "mx-2 mb-1 flex min-h-11 items-center justify-between rounded px-3 text-left text-sm text-fg-dim hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                onclick: move |_| {
                    close(());
                    palette_open.set(true);
                },
                span { "Jump to…" }
                kbd { class: "rounded border border-border-soft px-1.5 py-0.5 font-mono text-xs text-muted-foreground", "⌘K" }
            }
            button {
                class: "mx-2 mb-1 flex min-h-11 items-center rounded px-3 text-left text-sm text-fg-dim hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                onclick: move |_| toggle_theme(()),
                if is_dark() { "Light mode" } else { "Dark mode" }
            }
            button {
                class: "m-2 min-h-11 rounded px-3 text-left text-sm text-fg-dim hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none",
                onclick: move |_| async move {
                    let _ = crate::api::logout().await;
                    nav.push(Route::Login {});
                },
                "Log out"
            }
        }
    }
}

type NavGroup = (&'static str, Vec<(&'static str, Route)>);

fn nav_groups() -> Vec<NavGroup> {
    vec![
        (
            "Fleet",
            vec![
                ("Dashboard", Route::Dashboard {}),
                ("Targets", Route::targets()),
                ("Target filters", Route::target_filters()),
            ],
        ),
        (
            "Releases",
            vec![
                ("Distributions", Route::distributions()),
                ("Modules", Route::modules()),
                ("Rollouts", Route::rollouts()),
                ("Actions", Route::actions()),
            ],
        ),
        (
            "Configuration",
            vec![("Tags", Route::tags()), ("Types", Route::Types {})],
        ),
    ]
}

#[component]
fn NavLink(to: Route, label: String, on_navigate: EventHandler<()>) -> Element {
    rsx! {
        Link {
            to,
            class: "rounded px-3 py-2 text-sm text-fg-dim hover:bg-accent",
            // The current page is marked by an accent edge plus full-strength
            // text, not by accent-coloured text: the accent is capped at about
            // two visible uses per screen and the brand mark already spends one.
            active_class: "bg-accent text-foreground shadow-[inset_2px_0_0_var(--color-primary)]",
            onclick: move |_| on_navigate.call(()),
            "{label}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_groups_cover_every_operator_page() {
        let groups = nav_groups();
        assert_eq!(
            groups.iter().map(|(_, links)| links.len()).sum::<usize>(),
            9
        );
        assert_eq!(
            groups.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            ["Fleet", "Releases", "Configuration"]
        );
    }
}
