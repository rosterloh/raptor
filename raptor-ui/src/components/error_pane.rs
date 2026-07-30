use dioxus::prelude::*;

#[component]
pub fn ErrorPane(message: String, on_retry: EventHandler<()>) -> Element {
    rsx! {
        div { class: "rounded border border-err-border bg-err-bg/40 p-4 text-sm",
            p { class: "mb-2 text-err-fg", "{message}" }
            button {
                class: "rounded border border-err-border px-3 py-1 text-err-fg hover:bg-err-bg/40",
                onclick: move |_| on_retry.call(()),
                "Retry"
            }
        }
    }
}
