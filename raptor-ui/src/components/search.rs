use crate::components::ui::Input;
use dioxus::prelude::*;

/// Long enough that typing a word doesn't fire a query per letter, short enough
/// that pausing feels like it responded rather than lagged.
const DEBOUNCE_MS: u32 = 300;

#[component]
pub fn SearchBox(placeholder: String, on_search: EventHandler<String>) -> Element {
    let mut value = use_signal(String::new);
    let mut dispatched = use_signal(String::new);

    // Forward only terms the parent hasn't already been handed. The debounce and
    // Enter would otherwise both fire for the same text, costing a duplicate
    // fetch — and it makes the initial mount a no-op, since both signals start
    // empty and the parent already treats an empty query as "no filter".
    let mut dispatch = move |v: String| {
        if dispatched() != v {
            dispatched.set(v.clone());
            on_search.call(v);
        }
    };

    // Debounce by sleeping inside the resource: a keystroke restarts it, which
    // drops the pending sleep, so only the text the user paused on is forwarded.
    // Same trick as the target-filter preview — no debounce helper needed.
    use_resource(move || async move {
        let v = value();
        gloo_timers::future::TimeoutFuture::new(DEBOUNCE_MS).await;
        dispatch(v);
    });

    rsx! {
        Input {
            class: "w-64",
            r#type: "search",
            placeholder,
            value: "{value}",
            oninput: move |e: FormEvent| value.set(e.value()),
            // Enter still means "now" rather than waiting out the debounce.
            onkeydown: move |e: KeyboardEvent| {
                if e.key() == Key::Enter {
                    dispatch(value());
                }
            },
        }
    }
}
