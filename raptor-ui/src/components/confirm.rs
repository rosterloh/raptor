use crate::components::ui::Dialog;
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
            h3 { class: "mb-2 text-lg font-semibold text-zinc-100", "{title}" }
            p { class: "mb-4 text-sm text-zinc-400", "{message}" }
            div { class: "flex justify-end gap-2",
                button {
                    class: "rounded px-3 py-1.5 text-sm text-zinc-300 hover:bg-zinc-800",
                    onclick: move |_| open.set(false),
                    "Cancel"
                }
                button {
                    class: crate::components::BTN_DANGER,
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
