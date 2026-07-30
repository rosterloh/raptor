use crate::components::ui::Input;
use dioxus::prelude::*;

#[component]
pub fn SearchBox(placeholder: String, on_search: EventHandler<String>) -> Element {
    let mut value = use_signal(String::new);
    rsx! {
        Input {
            class: "w-64",
            r#type: "search",
            placeholder,
            value: "{value}",
            oninput: move |e: FormEvent| value.set(e.value()),
            onkeydown: move |e: KeyboardEvent| {
                if e.key() == Key::Enter {
                    on_search.call(value());
                }
            },
        }
    }
}
