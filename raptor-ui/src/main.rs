use dioxus::prelude::*;

mod api;
mod components;
mod logic;
mod pages;

use pages::{
    Actions, Dashboard, Distributions, DsDetail, Login, ModuleDetail, Modules, Shell, TargetDetail,
    Targets,
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
        #[route("/targets")]
        Targets {},
        #[route("/targets/:cid")]
        TargetDetail { cid: String },
        #[route("/distributions")]
        Distributions {},
        #[route("/distributions/:id")]
        DsDetail { id: i64 },
        #[route("/modules")]
        Modules {},
        #[route("/modules/:id")]
        ModuleDetail { id: i64 },
        #[route("/actions")]
        Actions {},
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
