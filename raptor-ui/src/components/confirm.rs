use crate::components::ui::{Button, ButtonVariant, Dialog};
use dioxus::prelude::*;

#[component]
pub fn ConfirmDialog(
    title: String,
    message: String,
    open: Signal<bool>,
    on_confirm: EventHandler<()>,
) -> Element {
    rsx! {
        Dialog { open,
            h3 { class: "mb-2 text-lg font-semibold text-foreground", "{title}" }
            p { class: "mb-4 text-sm text-fg-dim", "{message}" }
            div { class: "flex justify-end gap-2",
                button {
                    class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                    onclick: move |_| open.set(false),
                    "Cancel"
                }
                Button {
                    variant: ButtonVariant::Destructive,
                    onclick: move |_| {
                        open.set(false);
                        on_confirm.call(());
                    },
                    "Confirm"
                }
            }
        }
    }
}
