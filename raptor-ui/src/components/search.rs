use dioxus::prelude::*;

#[component]
pub fn SearchBox(placeholder: String, on_search: EventHandler<String>) -> Element {
    let mut value = use_signal(String::new);
    rsx! {
        input {
            class: "w-64 rounded border border-zinc-700 bg-zinc-900 px-3 py-1.5 text-sm text-zinc-200 placeholder-zinc-500 focus:border-emerald-600 focus:outline-none",
            r#type: "search",
            placeholder,
            value: "{value}",
            oninput: move |e| value.set(e.value()),
            onkeydown: move |e| {
                if e.key() == Key::Enter {
                    on_search.call(value());
                }
            },
        }
    }
}
