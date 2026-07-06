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
stub_page!(Actions);
