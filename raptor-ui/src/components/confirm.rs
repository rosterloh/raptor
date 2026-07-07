use dioxus::prelude::*;

#[component]
pub fn ConfirmDialog(
    title: String,
    message: String,
    open: Signal<bool>,
    on_confirm: EventHandler<()>,
) -> Element {
    rsx! {
        if open() {
            div { class: "fixed inset-0 z-40 flex items-center justify-center bg-black/60",
                div { class: "w-96 rounded-lg border border-zinc-800 bg-zinc-900 p-6",
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
    }
}
