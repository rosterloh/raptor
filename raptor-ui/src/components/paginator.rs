use dioxus::prelude::*;

#[component]
pub fn Paginator(offset: u64, limit: u64, total: u64, on_change: EventHandler<u64>) -> Element {
    let end = (offset + limit).min(total);
    let from = if total == 0 { 0 } else { offset + 1 };
    rsx! {
        div { class: "flex items-center justify-between py-2 text-sm text-zinc-400",
            span { "{from}–{end} of {total}" }
            div { class: "flex gap-2",
                button {
                    class: "rounded border border-zinc-700 px-2 py-1 disabled:opacity-40",
                    disabled: offset == 0,
                    onclick: move |_| on_change.call(offset.saturating_sub(limit)),
                    "Prev"
                }
                button {
                    class: "rounded border border-zinc-700 px-2 py-1 disabled:opacity-40",
                    disabled: offset + limit >= total,
                    onclick: move |_| on_change.call(offset + limit),
                    "Next"
                }
            }
        }
    }
}
