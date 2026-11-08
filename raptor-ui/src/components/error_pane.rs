use crate::components::ui::{Button, ButtonSize, ButtonVariant};
use dioxus::prelude::*;

#[component]
pub fn ErrorPane(message: String, on_retry: EventHandler<()>) -> Element {
    rsx! {
        div { class: "rounded border border-err-border bg-err-bg/40 p-4 text-sm",
            p { class: "mb-2 text-err-fg", "{message}" }
            Button {
                variant: ButtonVariant::Outline,
                size: ButtonSize::Touch,
                class: "border-err-border bg-transparent text-err-fg hover:bg-err-bg/40 hover:text-err-fg",
                onclick: move |_| on_retry.call(()),
                "Retry"
            }
        }
    }
}
