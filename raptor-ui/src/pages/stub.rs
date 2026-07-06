use crate::Route;
use dioxus::prelude::*;

macro_rules! stub_page {
    ($name:ident) => {
        #[component]
        pub fn $name() -> Element {
            rsx! { h1 { class: "text-xl font-bold", {stringify!($name)} } }
        }
    };
}

stub_page!(Login);
stub_page!(Dashboard);
stub_page!(Targets);
stub_page!(Distributions);
stub_page!(Modules);
stub_page!(Actions);

#[component]
pub fn TargetDetail(cid: String) -> Element {
    rsx! { h1 { "target {cid}" } }
}

#[component]
pub fn DsDetail(id: i64) -> Element {
    rsx! { h1 { "ds {id}" } }
}

#[component]
pub fn ModuleDetail(id: i64) -> Element {
    rsx! { h1 { "module {id}" } }
}

#[component]
pub fn Shell() -> Element {
    rsx! {
        div { class: "min-h-screen bg-zinc-950 text-zinc-200 p-6",
            Outlet::<Route> {}
        }
    }
}
