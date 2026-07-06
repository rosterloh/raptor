use dioxus::prelude::*;

macro_rules! stub_page {
    ($name:ident) => {
        #[component]
        pub fn $name() -> Element {
            rsx! { h1 { class: "text-xl font-bold", {stringify!($name)} } }
        }
    };
}

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
