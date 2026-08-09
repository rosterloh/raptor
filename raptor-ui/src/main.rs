//! Dioxus web app entry point: the `Route` enum (client-side routing) and
//! `main`/`App` bootstrap. Route components live in `pages`, HTTP calls in
//! `api`, and framework-independent logic in `logic`.

use dioxus::prelude::*;

mod api;
mod components;
mod logic;
mod pages;

use pages::{
    Actions, Dashboard, Distributions, DsDetail, Login, ModuleDetail, Modules, RolloutDetail,
    Rollouts, Shell, Tags, TargetDetail, TargetFilters, Targets,
};

const TAILWIND: Asset = asset!("/assets/tailwind.css");
const FAVICON: Asset = asset!("/assets/favicon.ico");

#[rustfmt::skip]
#[derive(Clone, Debug, PartialEq, Routable)]
pub enum Route {
    #[route("/login")]
    Login {},
    #[layout(Shell)]
        #[route("/")]
        Dashboard {},
        #[route("/targets?:query&:state&:tag&:offset")]
        Targets { query: String, state: String, tag: String, offset: u64 },
        #[route("/targets/:cid")]
        TargetDetail { cid: String },
        #[route("/targetfilters?:query&:offset")]
        TargetFilters { query: String, offset: u64 },
        #[route("/tags?:kind&:query&:offset")]
        Tags { kind: String, query: String, offset: u64 },
        #[route("/distributions?:query&:tag&:offset")]
        Distributions { query: String, tag: String, offset: u64 },
        #[route("/distributions/:id")]
        DsDetail { id: i64 },
        #[route("/modules?:query&:offset")]
        Modules { query: String, offset: u64 },
        #[route("/modules/:id")]
        ModuleDetail { id: i64 },
        #[route("/rollouts?:query&:offset")]
        Rollouts { query: String, offset: u64 },
        #[route("/rollouts/:id")]
        RolloutDetail { id: i64 },
        #[route("/actions?:filter&:offset")]
        Actions { filter: String, offset: u64 },
}

impl Route {
    // Every field falls back to `Default` via `FromQueryArgument`, so these
    // just spell out the bare-route form once instead of at every
    // nav-link/deep-link call site.
    pub fn targets() -> Self {
        Route::Targets {
            query: String::new(),
            state: String::new(),
            tag: String::new(),
            offset: 0,
        }
    }
    pub fn target_filters() -> Self {
        Route::TargetFilters {
            query: String::new(),
            offset: 0,
        }
    }
    pub fn tags() -> Self {
        Route::Tags {
            kind: String::new(),
            query: String::new(),
            offset: 0,
        }
    }
    pub fn distributions() -> Self {
        Route::Distributions {
            query: String::new(),
            tag: String::new(),
            offset: 0,
        }
    }
    pub fn modules() -> Self {
        Route::Modules {
            query: String::new(),
            offset: 0,
        }
    }
    pub fn rollouts() -> Self {
        Route::Rollouts {
            query: String::new(),
            offset: 0,
        }
    }
    pub fn actions() -> Self {
        Route::Actions {
            filter: String::new(),
            offset: 0,
        }
    }
}

fn main() {
    console_error_panic_hook::set_once();
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Stylesheet { href: TAILWIND }
        document::Link { rel: "icon", href: FAVICON }
        Router::<Route> {}
    }
}
