use crate::components::ToastStack;
use crate::Route;
use dioxus::prelude::*;

const LOGO: Asset = asset!("/assets/logo/logo-sidebar.png");

#[component]
pub fn Shell() -> Element {
    let nav = use_navigator();
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
        div { class: "flex min-h-screen bg-zinc-950 text-zinc-200",
            aside { class: "flex w-52 flex-col border-r border-zinc-800 bg-zinc-900",
                div { class: "flex items-center gap-2 px-4 py-5",
                    img { src: LOGO, class: "h-8 w-8", alt: "" }
                    span { class: "text-lg font-bold tracking-wide text-emerald-400", "raptor" }
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
                    class: "m-2 rounded px-3 py-2 text-left text-sm text-zinc-400 hover:bg-zinc-800",
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
    }
}

#[component]
fn NavLink(to: Route, label: String) -> Element {
    rsx! {
        Link {
            to,
            class: "rounded px-3 py-2 text-sm text-zinc-300 hover:bg-zinc-800",
            active_class: "bg-zinc-800 text-emerald-400",
            "{label}"
        }
    }
}
