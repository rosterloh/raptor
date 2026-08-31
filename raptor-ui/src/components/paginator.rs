use dioxus::prelude::*;

#[component]
pub fn Paginator(offset: u64, limit: u64, total: u64, on_change: EventHandler<u64>) -> Element {
    let end = (offset + limit).min(total);
    let from = if total == 0 { 0 } else { offset + 1 };
    let (page, pages, last_offset) = pagination_state(offset, limit, total);
    let button_class = "inline-flex min-h-11 min-w-11 items-center justify-center rounded border border-border px-3 text-sm text-foreground hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none disabled:pointer-events-none disabled:opacity-40";
    rsx! {
        div { class: "flex flex-wrap items-center justify-between gap-3 py-2 text-sm text-fg-dim",
            span { "{from}–{end} of {total} · page {page} of {pages}" }
            div { class: "flex gap-2", role: "group", aria_label: "Pagination",
                button {
                    class: button_class,
                    aria_label: "First page",
                    disabled: offset == 0,
                    onclick: move |_| on_change.call(0),
                    "First"
                }
                button {
                    class: button_class,
                    aria_label: "Previous page",
                    disabled: offset == 0,
                    onclick: move |_| on_change.call(offset.saturating_sub(limit)),
                    "Prev"
                }
                button {
                    class: button_class,
                    aria_label: "Next page",
                    disabled: offset + limit >= total,
                    onclick: move |_| on_change.call(offset + limit),
                    "Next"
                }
                button {
                    class: button_class,
                    aria_label: "Last page",
                    disabled: offset + limit >= total,
                    onclick: move |_| on_change.call(last_offset),
                    "Last"
                }
            }
        }
    }
}

fn pagination_state(offset: u64, limit: u64, total: u64) -> (u64, u64, u64) {
    if total == 0 || limit == 0 {
        return (0, 0, 0);
    }
    let pages = total.div_ceil(limit);
    let page = (offset / limit + 1).min(pages);
    (page, pages, (pages - 1) * limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_state_handles_empty_and_partial_last_pages() {
        assert_eq!(pagination_state(0, 25, 0), (0, 0, 0));
        assert_eq!(pagination_state(25, 25, 101), (2, 5, 100));
        assert_eq!(pagination_state(200, 25, 101), (5, 5, 100));
    }
}
