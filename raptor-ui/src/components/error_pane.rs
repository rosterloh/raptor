use dioxus::prelude::*;

#[component]
pub fn ErrorPane(message: String, on_retry: EventHandler<()>) -> Element {
    rsx! {
        div { class: "rounded border border-red-900 bg-red-950/40 p-4 text-sm",
            p { class: "mb-2 text-red-300", "{message}" }
            button {
                class: "rounded border border-red-800 px-3 py-1 text-red-200 hover:bg-red-900/40",
                onclick: move |_| on_retry.call(()),
                "Retry"
            }
        }
    }
}
